#pragma once
// Modelo de erro em dois níveis (ver docs/designs/subsystems/01_kernel_and_pcb.md §5).
//
//  Tier 1 — GuestTrap:   condições NORMAIS do SO simulado (page fault, OOM...).
//                        São dados, fazem parte da simulação, nunca são "bugs".
//  Tier 2 — EngineError: violações de invariantes do MOTOR (bugs ou política
//                        de utilizador defeituosa). Param a simulação.
//
// Regra prática ao escrever código novo:
//   "Um programa simulado bem escrito pode provocar isto?"
//     sim -> GuestTrap     não -> EngineError

#include <cstdint>
#include <string>
#include <string_view>
#include <variant>

#include "nvos/core/ids.hpp"

namespace nvos::core {

// ---------- Tier 1 ----------
struct PageFaultTrap     { std::uint32_t virtual_addr; bool operator==(const PageFaultTrap&) const = default; };
struct IllegalAccessTrap { std::uint32_t address;      bool operator==(const IllegalAccessTrap&) const = default; };
struct FileNotFoundTrap  { FileDescriptor fd;          bool operator==(const FileNotFoundTrap&) const = default; };
struct OutOfMemoryTrap   {                             bool operator==(const OutOfMemoryTrap&) const = default; };

using GuestTrap = std::variant<PageFaultTrap, IllegalAccessTrap, FileNotFoundTrap, OutOfMemoryTrap>;

// ---------- Tier 2 ----------
enum class EngineErrorKind {
  InvalidProcessHandle,
  ClockMonotonicityViolation,
  InvalidStateTransition,
  SubsystemFailure,
};

[[nodiscard]] std::string_view to_string(EngineErrorKind kind) noexcept;

struct EngineError {
  EngineErrorKind kind;
  std::string     message;  // contexto humano: o quê, onde, com que valores
};

}  // namespace nvos::core
