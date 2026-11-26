#include <catch2/catch_test_macros.hpp>

#include "nvos/core/clock.hpp"
#include "nvos/core/tick.hpp"

using namespace nvos::core;

TEST_CASE("Tick: ordenação e aritmética", "[core][tick]") {
  REQUIRE(Tick{3} < Tick{4});
  REQUIRE(Tick{3} + 2 == Tick{5});
  REQUIRE(Tick{9}.since(Tick{4}) == 5);
}

TEST_CASE("Clock: avança por duração e por salto", "[core][clock]") {
  Clock c;
  REQUIRE(c.now() == Tick{0});
  REQUIRE(c.advance_by(5).has_value());
  REQUIRE(c.advance_to(Tick{20}).has_value());
  REQUIRE(c.now() == Tick{20});
}

TEST_CASE("Clock: salto para o mesmo tick é um no-op válido", "[core][clock]") {
  Clock c;
  REQUIRE(c.advance_to(Tick{7}).has_value());
  REQUIRE(c.advance_to(Tick{7}).has_value());
  REQUIRE(c.now() == Tick{7});
}

TEST_CASE("Clock: recuar no tempo é erro de motor e não altera o estado", "[core][clock]") {
  Clock c;
  REQUIRE(c.advance_to(Tick{10}).has_value());

  auto r = c.advance_to(Tick{3});
  REQUIRE_FALSE(r.has_value());
  REQUIRE(r.error().kind == EngineErrorKind::ClockMonotonicityViolation);
  REQUIRE(c.now() == Tick{10});
}

TEST_CASE("Clock: overflow é detetado", "[core][clock]") {
  Clock c;
  REQUIRE(c.advance_to(Tick{UINT64_MAX}).has_value());
  REQUIRE_FALSE(c.advance_by(1).has_value());
}
