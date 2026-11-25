# Dependencias primcipais. Sem Processo.
include(FetchContent)
FetchContent_Declare(Catch2        GIT_REPOSITORY https://github.com/catchorg/Catch2.git        GIT_TAG v3.7.1)
FetchContent_Declare(nlohmann_json GIT_REPOSITORY https://github.com/nlohmann/json.git          GIT_TAG v3.11.3)
FetchContent_Declare(tomlplusplus  GIT_REPOSITORY https://github.com/marzer/tomlplusplus.git    GIT_TAG v3.4.0)
FetchContent_MakeAvailable(Catch2 nlohmann_json tomlplusplus)
