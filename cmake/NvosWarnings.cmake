function(nvos_set_warnings target)
  if(MSVC)
    target_compile_options(${target} PRIVATE /W4 /WX /fp:strict)
  else()
    target_compile_options(${target} PRIVATE
      -Wall -Wextra -Wpedantic -Wconversion -Wshadow -Werror
      -ffp-contract=off)            # evita FMA, que muda resultados entre CPUs
    if(NVOS_SANITIZE)
      target_compile_options(${target} PRIVATE -fsanitize=address,undefined)
      target_link_options(${target} PRIVATE -fsanitize=address,undefined)
    endif()
  endif()
endfunction()
