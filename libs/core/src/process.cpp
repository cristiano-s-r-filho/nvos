#include "nvos/core/process.hpp"

#include <array>
#include <utility>

namespace nvos::core {

std::string_view to_string(StateKind kind) noexcept {
  switch (kind) {
    case StateKind::New:        return "New";
    case StateKind::Ready:      return "Ready";
    case StateKind::Running:    return "Running";
    case StateKind::Blocked:    return "Blocked";
    case StateKind::Sleeping:   return "Sleeping";
    case StateKind::Terminated: return "Terminated";
  }
  return "Unknown";
}

namespace {

constexpr std::size_t kStateCount = 6;

// Tabela de adjacência [de][para]. Ler por linhas: "de New posso ir para...".
// Manter como dados (e não como if/else) torna a regra visível e fácil de rever.
//                                           New    Ready  Run    Block  Sleep  Term
constexpr std::array<std::array<bool, kStateCount>, kStateCount> kTransitions{{
    /* New        */ {{false, true,  false, false, false, true }},
    /* Ready      */ {{false, false, true,  false, false, true }},
    /* Running    */ {{false, true,  false, true,  true,  true }},
    /* Blocked    */ {{false, true,  false, false, false, true }},
    /* Sleeping   */ {{false, true,  false, false, false, true }},
    /* Terminated */ {{false, false, false, false, false, false}},
}};

}  // namespace

bool is_valid_transition(StateKind from, StateKind to) noexcept {
  return kTransitions[static_cast<std::size_t>(from)][static_cast<std::size_t>(to)];
}

ProcessControlBlock make_pcb(Pid pid, std::string name, Tick arrival,
                             PriorityLevel priority,
                             std::vector<InstructionToken> tape) {
  ProcessControlBlock pcb;
  pcb.pid            = pid;
  pcb.name           = std::move(name);
  pcb.priority       = priority;
  pcb.base_priority  = priority;
  pcb.arrival_tick   = arrival;
  pcb.execution_tape = std::move(tape);
  return pcb;  // state já é New{}
}

std::expected<void, EngineError> transition_to(ProcessControlBlock& pcb, ProcessState next) {
  const StateKind from = kind_of(pcb.state);
  const StateKind to   = kind_of(next);
  if (!is_valid_transition(from, to)) {
    return std::unexpected(EngineError{
        EngineErrorKind::InvalidStateTransition,
        "PID " + std::to_string(pcb.pid.value) + ": " +
            std::string(to_string(from)) + " -> " + std::string(to_string(to)) +
            " não é permitido"});
  }
  pcb.state = std::move(next);
  return {};
}

}  // namespace nvos::core
