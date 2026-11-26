#pragma once
// Clock: Relogio de sistema unico, sem calculos complexos.

#include <cstdint>
#include <expected>
#include <limits>
#include <string>

#include "nvos/core/error.hpp"
#include "nvos/core/tick.hpp"

namespace nvos::core {

class Clock {
 public:
  [[nodiscard]] Tick now() const noexcept { return now_; }

  // Salta para `target` (usado quando a CPU está idle e há um evento futuro).
  // Recuar é um bug do motor; igualdade é um no-op válido.
  [[nodiscard]] std::expected<void, EngineError> advance_to(Tick target) {
    if (target < now_) {
      return std::unexpected(EngineError{
          EngineErrorKind::ClockMonotonicityViolation,
          "current=" + std::to_string(now_.value) +
              " attempted=" + std::to_string(target.value)});
    }
    now_ = target;
    return {};
  }

  // Avança `duration` ticks (usado durante bursts de CPU, tick a tick).
  [[nodiscard]] std::expected<void, EngineError> advance_by(std::uint64_t duration) {
    if (duration > std::numeric_limits<std::uint64_t>::max() - now_.value) {
      return std::unexpected(EngineError{
          EngineErrorKind::ClockMonotonicityViolation,
          "overflow ao avançar " + std::to_string(duration) + " ticks"});
    }
    now_ = now_ + duration;
    return {};
  }

 private:
  Tick now_{};
};

}  // namespace nvos::core
