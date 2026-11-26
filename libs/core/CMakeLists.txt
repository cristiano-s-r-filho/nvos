# nvos::core — primitivos partilhados por todos os subsistemas.
# Regra: esta biblioteca NÃO depende de nenhuma outra (nem de terceiros).
nvos_add_library(core
  SOURCES
    src/error.cpp
    src/process.cpp
  DEPS "")

if(NVOS_BUILD_TESTS)
  nvos_add_test(nvos_core_tests
    SOURCES
      tests/tick_clock_test.cpp
      tests/arena_test.cpp
      tests/process_test.cpp
    DEPS nvos::core)
endif()
