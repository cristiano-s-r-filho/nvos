#include "nvos/core/error.hpp"

namespace nvos::core {

std::string_view to_string(EngineErrorKind kind) noexcept {
  switch (kind) {
    case EngineErrorKind::InvalidProcessHandle:        return "InvalidProcessHandle";
    case EngineErrorKind::ClockMonotonicityViolation:  return "ClockMonotonicityViolation";
    case EngineErrorKind::InvalidStateTransition:      return "InvalidStateTransition";
    case EngineErrorKind::SubsystemFailure:            return "SubsystemFailure";
  }
  return "Unknown";  // inalcançável; mantém o compilador feliz
}

}  // namespace nvos::core
