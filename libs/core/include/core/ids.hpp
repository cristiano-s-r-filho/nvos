#pragma once
// Identificadores opacos ("strong typedefs").
//
// PORQUÊ: o PCB guarda handles para subsistemas (memória, FS, I/O) sem os
// conhecer. Se Pid e FileDescriptor fossem ambos `uint32_t`, trocá-los
// compilaria sem erro. Com StrongId, é um erro de compilação.
//
// NOTA DE DETERMINISMO: propositadamente NÃO existe std::hash<StrongId>.
// Isto impede o uso acidental de unordered_map; usa std::map (ordenado).

#include <compare>
#include <cstdint>

namespace nvos::core {

template <class Tag, class Rep = std::uint32_t>
struct StrongId {
  Rep value{};

  constexpr StrongId() = default;
  constexpr explicit StrongId(Rep v) noexcept : value(v) {}

  friend constexpr auto operator<=>(const StrongId&, const StrongId&) = default;
};

struct PidTag {};
struct PriorityTag {};
struct PageTableTag {};
struct HeapBlockTag {};
struct FileDescriptorTag {};
struct InodeTag {};

using Pid            = StrongId<PidTag>;
using PriorityLevel  = StrongId<PriorityTag, std::uint8_t>;  // 0 = mais alta
using PageTableId    = StrongId<PageTableTag>;
using HeapBlockId    = StrongId<HeapBlockTag>;
using FileDescriptor = StrongId<FileDescriptorTag>;
using InodeId        = StrongId<InodeTag>;

}  // namespace nvos::core
