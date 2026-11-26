#pragma once
// Process Control Block (PCB), estados do processo e fita de instruções.
//
// Estrutura deste ficheiro:
//   1. Razões de bloqueio / saída
//   2. Estados + tabela de transições válidas      <- a máquina de estados
//   3. Instruções da fita (o "programa" sintético)
//   4. O PCB e a ÚNICA função que muda o estado    (transition_to)
//
// Convenção: os alternativos de variant são structs pequenos com nome próprio,
// para `std::visit` ficar legível e o compilador avisar se esqueceres um caso.

#include <cstddef>
#include <cstdint>
#include <expected>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <type_traits>
#include <variant>
#include <vector>

#include "nvos/core/error.hpp"
#include "nvos/core/ids.hpp"
#include "nvos/core/tick.hpp"

namespace nvos::core {

// ---------- 1. Razões ----------
struct BlockedOnIo        { std::uint32_t device_id; std::uint64_t request_id; bool operator==(const BlockedOnIo&) const = default; };
struct BlockedOnPageFault { std::uint32_t virtual_addr;                        bool operator==(const BlockedOnPageFault&) const = default; };
using BlockReason = std::variant<BlockedOnIo, BlockedOnPageFault>;

struct ExitNormal            { std::int32_t code;    bool operator==(const ExitNormal&) const = default; };
struct ExitSegfault          { std::uint32_t address; bool operator==(const ExitSegfault&) const = default; };
struct ExitIllegalInstruction{                         bool operator==(const ExitIllegalInstruction&) const = default; };
struct ExitQuantumTrap       {                         bool operator==(const ExitQuantumTrap&) const = default; };
struct ExitKilled            {                         bool operator==(const ExitKilled&) const = default; };
using ExitReason = std::variant<ExitNormal, ExitSegfault, ExitIllegalInstruction, ExitQuantumTrap, ExitKilled>;

// ---------- 2. Estados ----------
struct New      { bool operator==(const New&) const = default; };
struct Ready    { bool operator==(const Ready&) const = default; };
struct Running  { bool operator==(const Running&) const = default; };
struct Blocked  { BlockReason reason; bool operator==(const Blocked&) const = default; };
struct Sleeping { Tick wake_at;       bool operator==(const Sleeping&) const = default; };
struct Terminated { ExitReason reason; bool operator==(const Terminated&) const = default; };
using ProcessState = std::variant<New, Ready, Running, Blocked, Sleeping, Terminated>;

// "Tipo" do estado, sem payload. Útil para tabelas, logs e a UI (cores do Gantt).
// A ordem DEVE coincidir com a de ProcessState (garantido por static_assert).
enum class StateKind : std::uint8_t { New, Ready, Running, Blocked, Sleeping, Terminated };

[[nodiscard]] constexpr StateKind kind_of(const ProcessState& s) noexcept {
  return static_cast<StateKind>(s.index());
}
static_assert(std::is_same_v<std::variant_alternative_t<static_cast<std::size_t>(StateKind::Terminated), ProcessState>, Terminated>,
              "StateKind e ProcessState têm de estar na mesma ordem");

[[nodiscard]] std::string_view to_string(StateKind kind) noexcept;

// Transições permitidas (qualquer estado não-terminal pode ir para Terminated,
// p.ex. por kill ou segfault):
//
//   New ─▶ Ready ─▶ Running ─▶ Ready      (preempção / yield)
//                      │  ├──▶ Blocked ─▶ Ready
//                      │  └──▶ Sleeping ─▶ Ready
//                      └─────▶ Terminated
[[nodiscard]] bool is_valid_transition(StateKind from, StateKind to) noexcept;

// ---------- 3. Fita de instruções ----------
enum class FaultTarget : std::uint8_t { DiskReadError, MemoryEccError, SpuriousInterrupt };

struct Cpu         { std::uint32_t ticks;                         bool operator==(const Cpu&) const = default; };
struct Alloc       { std::size_t bytes;                           bool operator==(const Alloc&) const = default; };
struct Free        { HeapBlockId block_id;                        bool operator==(const Free&) const = default; };
struct MemoryRead  { std::uint32_t virtual_addr;                  bool operator==(const MemoryRead&) const = default; };
struct MemoryWrite { std::uint32_t virtual_addr;                  bool operator==(const MemoryWrite&) const = default; };
struct FileRead    { FileDescriptor fd; std::size_t bytes;        bool operator==(const FileRead&) const = default; };
struct FileWrite   { FileDescriptor fd; std::size_t bytes;        bool operator==(const FileWrite&) const = default; };
struct Sleep       { std::uint32_t ticks;                         bool operator==(const Sleep&) const = default; };
struct FaultInject { FaultTarget target; std::uint32_t error_code; bool operator==(const FaultInject&) const = default; };
struct Exit        { std::int32_t code;                           bool operator==(const Exit&) const = default; };

using InstructionToken = std::variant<Cpu, Alloc, Free, MemoryRead, MemoryWrite,
                                      FileRead, FileWrite, Sleep, FaultInject, Exit>;

// ---------- 4. PCB ----------
// É um agregado de DADOS. A lógica de execução (avançar a fita, quantum, etc.)
// vive no kernel (Fase 1), não aqui: assim o core fica pequeno e estável.
struct ProcessControlBlock {
  // Identificação
  Pid                  pid;
  std::optional<Pid>   ppid;
  std::string          name;

  // Estado e prioridade
  ProcessState         state{New{}};
  PriorityLevel        priority;
  PriorityLevel        base_priority;
  std::uint32_t        quantum_remaining{0};

  // Contabilidade de CPU
  std::size_t          program_counter{0};          // índice na execution_tape
  std::uint32_t        current_token_progress{0};   // ticks já gastos no token atual
  std::uint64_t        cpu_time_consumed{0};
  Tick                 arrival_tick;
  std::optional<Tick>  completion_tick;

  // Handles opacos para outros subsistemas (o core não os conhece)
  std::optional<PageTableId>             page_table_root;
  std::vector<HeapBlockId>               allocated_heap_blocks;
  std::map<FileDescriptor, InodeId>      file_descriptors;  // map: ordem determinística

  // O "programa"
  std::vector<InstructionToken>          execution_tape;

  bool operator==(const ProcessControlBlock&) const = default;
};

// Cria um PCB no estado New com os campos obrigatórios.
[[nodiscard]] ProcessControlBlock make_pcb(Pid pid, std::string name, Tick arrival,
                                           PriorityLevel priority,
                                           std::vector<InstructionToken> tape);

// ÚNICO ponto onde `pcb.state` deve ser alterado. Garante a máquina de estados.
// Erro: EngineErrorKind::InvalidStateTransition (é um bug do kernel, não do guest).
[[nodiscard]] std::expected<void, EngineError> transition_to(ProcessControlBlock& pcb, ProcessState next);

}  // namespace nvos::core
