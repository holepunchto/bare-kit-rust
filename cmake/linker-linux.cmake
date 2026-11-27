# Run in place of the linker for the relocatable link, leaving out the system
# libraries. As `-r` implies `-Bstatic`, the link would otherwise copy members
# of their static archives into the output. Their names go into
# `bare_kit.libs` instead, for the final link to link dynamically.

set(args)
set(unparsed OFF)

math(EXPR last "${CMAKE_ARGC} - 1")

foreach(i RANGE ${last})
  set(arg "${CMAKE_ARGV${i}}")

  if(unparsed)
    if(arg MATCHES "^@(.+)$")
      file(READ "${CMAKE_MATCH_1}" content)

      separate_arguments(content UNIX_COMMAND "${content}")

      list(APPEND args ${content})
    else()
      list(APPEND args "${arg}")
    endif()
  elseif(arg STREQUAL "--")
    set(unparsed ON)
  endif()
endforeach()

set(command)
set(libraries)

foreach(arg IN LISTS args)
  if(arg MATCHES "^-l(.+)$")
    list(APPEND libraries "${CMAKE_MATCH_1}")
  else()
    list(APPEND command "${arg}")
  endif()
endforeach()

list(REMOVE_DUPLICATES libraries)

execute_process(
  COMMAND ${command}
  COMMAND_ERROR_IS_FATAL ANY
)

list(TRANSFORM libraries APPEND "\n")
list(JOIN libraries "" libraries)

file(WRITE "${OUTPUT_DIR}/bare_kit.libs" "${libraries}")
