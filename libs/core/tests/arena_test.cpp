#include <catch2/catch_test_macros.hpp>

#include <string>
#include <vector>

#include "nvos/core/arena.hpp"

using nvos::core::Arena;
using nvos::core::Handle;

TEST_CASE("Arena: insert/get/size", "[core][arena]") {
  Arena<std::string> a;
  REQUIRE(a.empty());

  Handle h = a.insert("init");
  REQUIRE(a.size() == 1);
  REQUIRE(a.contains(h));
  REQUIRE(*a.get(h) == "init");
}

TEST_CASE("Arena: remove devolve o valor e invalida o handle", "[core][arena]") {
  Arena<int> a;
  Handle h = a.insert(42);

  auto removed = a.remove(h);
  REQUIRE(removed.has_value());
  REQUIRE(*removed == 42);
  REQUIRE(a.get(h) == nullptr);
  REQUIRE_FALSE(a.remove(h).has_value());  // remover duas vezes é seguro
  REQUIRE(a.size() == 0);
}

TEST_CASE("Arena: handle antigo NÃO acede ao objeto que reutilizou o slot (ABA)", "[core][arena]") {
  Arena<int> a;
  Handle old_h = a.insert(1);
  a.remove(old_h);
  Handle new_h = a.insert(2);

  REQUIRE(new_h.index == old_h.index);            // mesmo slot...
  REQUIRE(new_h.generation != old_h.generation);  // ...geração diferente
  REQUIRE(a.get(old_h) == nullptr);
  REQUIRE(*a.get(new_h) == 2);
}

TEST_CASE("Arena: reutilização de slots é LIFO (determinismo)", "[core][arena][determinism]") {
  Arena<int> a;
  Handle h0 = a.insert(0);
  Handle h1 = a.insert(1);
  Handle h2 = a.insert(2);
  (void)h2;

  a.remove(h1);  // libertado primeiro
  a.remove(h0);  // libertado por último -> reutilizado primeiro

  REQUIRE(a.insert(10).index == h0.index);
  REQUIRE(a.insert(11).index == h1.index);
}

TEST_CASE("Arena: for_each percorre por índice crescente, só os vivos", "[core][arena][determinism]") {
  Arena<int> a;
  Handle h0 = a.insert(100);
  Handle h1 = a.insert(101);
  Handle h2 = a.insert(102);
  (void)h0; (void)h2;
  a.remove(h1);

  std::vector<int> seen;
  a.for_each([&](Handle, int& v) { seen.push_back(v); });
  REQUIRE(seen == std::vector<int>{100, 102});
}

TEST_CASE("Arena: handle fora de limites é inválido, não UB", "[core][arena]") {
  Arena<int> a;
  REQUIRE(a.get(Handle{999, 0}) == nullptr);
  REQUIRE_FALSE(a.contains(Handle{0, 0}));
}
