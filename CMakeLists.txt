cmake_minimum_required(VERSION 3.25)
project(nvos VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

option(NVOS_BUILD_TESTS "Build tests" ON)
option(NVOS_BUILD_TUI   "Build TUI"   ON)
option(NVOS_BUILD_WEB   "Build web server" ON)
option(NVOS_SANITIZE    "ASan+UBSan"  OFF)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
include(NvosWarnings)
include(NvosDependencies)
include(NvosTargets)

if(NVOS_BUILD_TESTS)
  enable_testing()
endif()

foreach(lib core cpu mem fs io trace scenario kernel)
  add_subdirectory(libs/${lib})
endforeach()

if(NVOS_BUILD_TUI) add_subdirectory(apps/tui) endif()
if(NVOS_BUILD_WEB) add_subdirectory(apps/web) endif()
add_subdirectory(apps/cli)
