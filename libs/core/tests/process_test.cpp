#include <catch2/catch_test_macros.hpp>

#include "nvos/core/process.hpp"

using namespace nvos::core;

namespace {
ProcessControlBlock sample_pcb() {
  return make_pcb(Pid{1}, "init", Tick{0}, PriorityLevel{0},
                  {Cpu{3}, Sleep{2}, Exit{0}});
}
}  // namespace

TEST_CASE("make_pcb: nasce em New com a prioridade-base igual à inicial", "[core][pcb]") {
  auto pcb = sample_pcb();
  REQUIRE(kind_of(pcb.state) == StateKind::New);
  REQUIRE(pcb.priority == pcb.base_priority);
  REQUIRE(pcb.program_counter == 0);
  REQUIRE(pcb.execution_tape.size() == 3);
}

TEST_CASE("kind_of coincide com a alternativa ativa do variant", "[core][pcb]") {
  REQUIRE(kind_of(ProcessState{New{}})        == StateKind::New);
  REQUIRE(kind_of(ProcessState{Ready{}})      == StateKind::Ready);
  REQUIRE(kind_of(ProcessState{Running{}})    == StateKind::Running);
  REQUIRE(kind_of(ProcessState{Blocked{BlockedOnPageFault{0x10}}}) == StateKind::Blocked);
  REQUIRE(kind_of(ProcessState{Sleeping{Tick{5}}}) == StateKind::Sleeping);
  REQUIRE(kind_of(ProcessState{Terminated{ExitNormal{0}}}) == StateKind::Terminated);
}

TEST_CASE("Ciclo de vida feliz: New -> Ready -> Running -> Blocked -> Ready -> Running -> Terminated",
          "[core][pcb][lifecycle]") {
  auto pcb = sample_pcb();
  REQUIRE(transition_to(pcb, Ready{}));
  REQUIRE(transition_to(pcb, Running{}));
  REQUIRE(transition_to(pcb, Blocked{BlockedOnIo{1, 77}}));
  REQUIRE(transition_to(pcb, Ready{}));
  REQUIRE(transition_to(pcb, Running{}));
  REQUIRE(transition_to(pcb, Terminated{ExitNormal{0}}));
  REQUIRE(kind_of(pcb.state) == StateKind::Terminated);
}

TEST_CASE("Transições inválidas são rejeitadas e o estado não muda", "[core][pcb][lifecycle]") {
  auto pcb = sample_pcb();

  auto r = transition_to(pcb, Running{});  // New -> Running: não permitido
  REQUIRE_FALSE(r.has_value());
  REQUIRE(r.error().kind == EngineErrorKind::InvalidStateTransition);
  REQUIRE(kind_of(pcb.state) == StateKind::New);
}

TEST_CASE("Terminated é absorvente", "[core][pcb][lifecycle]") {
  auto pcb = sample_pcb();
  REQUIRE(transition_to(pcb, Terminated{ExitKilled{}}));  // New -> Terminated (kill)
  for (auto next : {ProcessState{Ready{}}, ProcessState{Running{}}, ProcessState{New{}}}) {
    REQUIRE_FALSE(transition_to(pcb, next).has_value());
  }
}

TEST_CASE("Tabela de transições: propriedades globais", "[core][pcb][lifecycle]") {
  constexpr StateKind all[] = {StateKind::New, StateKind::Ready, StateKind::Running,
                               StateKind::Blocked, StateKind::Sleeping, StateKind::Terminated};
  for (auto s : all) {
    // Nenhum estado volta a New e nenhum estado se "transita" para si próprio.
    REQUIRE_FALSE(is_valid_transition(s, StateKind::New));
    REQUIRE_FALSE(is_valid_transition(s, s));
    // Todo o estado não-terminal pode terminar (kill/segfault).
    if (s != StateKind::Terminated) REQUIRE(is_valid_transition(s, StateKind::Terminated));
  }
  // Só Running pode bloquear ou adormecer.
  for (auto s : all) {
    if (s != StateKind::Running) {
      REQUIRE_FALSE(is_valid_transition(s, StateKind::Blocked));
      REQUIRE_FALSE(is_valid_transition(s, StateKind::Sleeping));
    }
  }
}
