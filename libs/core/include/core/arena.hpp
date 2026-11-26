#pragma once
// Arena<T>: armazenamento com handles geracionais.
//
// PORQUÊ (ver concepts_brief.md §2): o kernel é o ÚNICO dono dos PCBs.
// Os subsistemas guardam apenas Handle/Pid, nunca ponteiros nem shared_ptr.
// A geração impede o bug clássico "ABA": um handle antigo apontar, sem erro,
// para um objeto novo que reutilizou o mesmo slot.
//
// GARANTIAS DE DETERMINISMO:
//   * reutilização de slots é LIFO (o último libertado é o primeiro reutilizado);
//   * for_each percorre sempre por índice crescente.
// Isto torna a ordem de iteração função apenas da sequência de operações.
//
// NÃO é thread-safe: pertence a uma única thread (a do kernel).
//
// Limitação conhecida: a geração é uint32 e dá a volta após 2^32 remoções
// do mesmo slot. Irrelevante para uma simulação; documentado por honestidade.

#include <cassert>
#include <compare>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <utility>
#include <vector>

namespace nvos::core {

struct Handle {
  std::uint32_t index{};
  std::uint32_t generation{};
  friend constexpr auto operator<=>(const Handle&, const Handle&) = default;
};

template <class T>
class Arena {
 public:
  // Insere e devolve um handle novo. Amortizado O(1).
  [[nodiscard]] Handle insert(T value) {
    std::uint32_t index;
    if (!free_.empty()) {
      index = free_.back();
      free_.pop_back();
    } else {
      assert(slots_.size() < std::numeric_limits<std::uint32_t>::max());
      index = static_cast<std::uint32_t>(slots_.size());
      slots_.emplace_back();
    }
    Slot& slot = slots_[index];
    slot.value.emplace(std::move(value));
    ++live_;
    return Handle{index, slot.generation};
  }

  // Remove e devolve o valor; nullopt se o handle for inválido/antigo.
  std::optional<T> remove(Handle h) {
    Slot* slot = find_slot(h);
    if (slot == nullptr) return std::nullopt;
    std::optional<T> out = std::move(slot->value);
    slot->value.reset();
    ++slot->generation;  // invalida TODOS os handles antigos deste slot
    free_.push_back(h.index);
    --live_;
    return out;
  }

  // nullptr se o handle for inválido. O ponteiro só vale até à próxima
  // inserção (o vetor interno pode realocar) — não o guardes.
  [[nodiscard]] T* get(Handle h) noexcept {
    Slot* slot = find_slot(h);
    return slot ? &*slot->value : nullptr;
  }
  [[nodiscard]] const T* get(Handle h) const noexcept {
    const Slot* slot = find_slot(h);
    return slot ? &*slot->value : nullptr;
  }

  [[nodiscard]] bool        contains(Handle h) const noexcept { return find_slot(h) != nullptr; }
  [[nodiscard]] std::size_t size() const noexcept { return live_; }
  [[nodiscard]] bool        empty() const noexcept { return live_ == 0; }

  // Visita elementos vivos por índice crescente: f(Handle, T&).
  // Não insiras nem removas dentro de `f`.
  template <class F>
  void for_each(F&& f) {
    for (std::uint32_t i = 0; i < slots_.size(); ++i) {
      if (slots_[i].value) f(Handle{i, slots_[i].generation}, *slots_[i].value);
    }
  }
  template <class F>
  void for_each(F&& f) const {
    for (std::uint32_t i = 0; i < slots_.size(); ++i) {
      if (slots_[i].value) f(Handle{i, slots_[i].generation}, *slots_[i].value);
    }
  }

 private:
  struct Slot {
    std::optional<T> value;
    std::uint32_t    generation{0};
  };

  Slot* find_slot(Handle h) noexcept {
    return const_cast<Slot*>(std::as_const(*this).find_slot(h));
  }
  const Slot* find_slot(Handle h) const noexcept {
    if (h.index >= slots_.size()) return nullptr;
    const Slot& slot = slots_[h.index];
    return (slot.value && slot.generation == h.generation) ? &slot : nullptr;
  }

  std::vector<Slot>          slots_;
  std::vector<std::uint32_t> free_;
  std::size_t                live_{0};
};

}  // namespace nvos::core
