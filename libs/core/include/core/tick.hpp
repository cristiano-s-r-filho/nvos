#pragma once
// Tick: a unidade de tempo virtual. Nunca existe tempo "real" no engine.

#include <cassert>
#include <compare>
#include <cstdint>

namespace nvos::core {

struct Tick {
  std::uint64_t value{};

  constexpr Tick() = default;
  constexpr explicit Tick(std::uint64_t v) noexcept : value(v) {}

  friend constexpr auto operator<=>(const Tick&, const Tick&) = default;

  // Tick + duração = Tick. Não há Tick + Tick (não faz sentido físico).
  [[nodiscard]] constexpr Tick operator+(std::uint64_t duration) const noexcept {
    return Tick{value + duration};
  }

  // Duração decorrida desde `earlier`. Pré-condição: earlier <= *this.
  [[nodiscard]] constexpr std::uint64_t since(Tick earlier) const noexcept {
    assert(earlier <= *this && "Tick::since: o passado não pode ser posterior");
    return value - earlier.value;
  }
};

}  // namespace nvos::core
