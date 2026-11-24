# Run in place of the MSVC linker, merging the inputs of the link into static
# libraries. The objects and the archives linked whole go into `bare_kit.lib`,
# which must be linked whole too, the other archives into `bare_kit_deps.lib`,
# and the names of the system libraries into `bare_kit.libs`.

set(args)
set(unparsed OFF)

math(EXPR last "${CMAKE_ARGC} - 1")

foreach(i RANGE ${last})
  set(arg "${CMAKE_ARGV${i}}")

  if(unparsed)
    if(arg MATCHES "^@(.+)$")
      file(READ "${CMAKE_MATCH_1}" content)

      separate_arguments(content WINDOWS_COMMAND "${content}")

      list(APPEND args ${content})
    else()
      list(APPEND args "${arg}")
    endif()
  elseif(arg STREQUAL "--")
    set(unparsed ON)
  endif()
endforeach()

set(output)
set(objects)
set(archives)
set(libraries)

foreach(arg IN LISTS args)
  if(arg MATCHES "^[/-]out:(.+)$")
    set(output "${CMAKE_MATCH_1}")
  elseif(arg MATCHES "^[/-]WHOLEARCHIVE:(.+)$")
    list(APPEND objects "${CMAKE_MATCH_1}")
  elseif(arg MATCHES "^[/-]DEFAULTLIB:(.+)$")
    list(APPEND libraries "${CMAKE_MATCH_1}")
  elseif(arg MATCHES "^[/-]")
    continue()
  elseif(arg MATCHES "\\.obj$")
    list(APPEND objects "${arg}")
  elseif(arg MATCHES "\\.lib$")
    cmake_path(ABSOLUTE_PATH arg BASE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}" OUTPUT_VARIABLE path)

    if(EXISTS "${path}")
      list(APPEND archives "${path}")
    else()
      list(APPEND libraries "${arg}")
    endif()
  endif()
endforeach()

list(REMOVE_DUPLICATES objects)
list(REMOVE_DUPLICATES archives)

list(TRANSFORM libraries REPLACE "\\.lib$" "")
list(REMOVE_DUPLICATES libraries)

foreach(name IN ITEMS bare_kit bare_kit_deps)
  if(name STREQUAL "bare_kit")
    set(inputs ${objects})
  else()
    set(inputs ${archives})
  endif()

  list(TRANSFORM inputs REPLACE "^(.+)$" "\"\\1\"")
  list(JOIN inputs "\n" inputs)

  file(WRITE "${OUTPUT_DIR}/${name}.rsp" "${inputs}\n")

  execute_process(
    COMMAND "${AR}" /nologo "/out:${OUTPUT_DIR}/${name}.lib" "@${OUTPUT_DIR}/${name}.rsp"
    COMMAND_ERROR_IS_FATAL ANY
  )
endforeach()

list(JOIN libraries "\n" libraries)

file(WRITE "${OUTPUT_DIR}/bare_kit.libs" "${libraries}\n")

file(TOUCH "${output}")
