cmake_minimum_required(VERSION 4.0)

find_package(cmake-bare REQUIRED PATHS node_modules/cmake-bare)
find_package(cmake-fetch REQUIRED PATHS node_modules/cmake-fetch)

project(bare_kit_rust C)

fetch_package("github:holepunchto/bare-kit@3.2.1")

bare_target(target)

# Executables only export what the linker is told to, so on Windows the exports
# of Bare and Bare Kit are embedded as linker directives in an object of their
# own. Elsewhere, the object is empty and the exports come from the link.
if(WIN32)
  set(exports "$<TARGET_PROPERTY:bare_kit::exports,INTERFACE_EXPORTED_SYMBOLS>")
  set(exports "$<LIST:TRANSFORM,${exports},PREPEND,#pragma comment(linker$<COMMA> \"/export:>")
  set(exports "$<LIST:TRANSFORM,${exports},APPEND,\")>")
  set(exports "$<JOIN:${exports},\n>")
else()
  set(exports)
endif()

set(exports_file "${CMAKE_CURRENT_BINARY_DIR}/exports.c")

file(GENERATE OUTPUT "${exports_file}" CONTENT "${exports}\n")

add_executable(bare_kit_prelink "${exports_file}")

target_link_libraries(
  bare_kit_prelink
  PRIVATE
    $<LINK_LIBRARY:WHOLE_ARCHIVE,bare_kit::static>
    bare_kit::exports
)

if(WIN32)
  # The MSVC linker cannot link relocatably, so the inputs of the link are
  # instead merged into static libraries.
  set(launcher
    "${CMAKE_COMMAND}"
    "-DAR=${CMAKE_AR}"
    "-DOUTPUT_DIR=${CMAKE_CURRENT_BINARY_DIR}"
    -P "${CMAKE_CURRENT_LIST_DIR}/cmake/archive.cmake"
    --
  )

  set_target_properties(
    bare_kit_prelink
    PROPERTIES
    C_LINKER_LAUNCHER "${launcher}"
    CXX_LINKER_LAUNCHER "${launcher}"
  )

  target_link_options(
    bare_kit_prelink
    PRIVATE
      /MANIFEST:NO # The launcher writes no executable to embed a manifest in
  )

  install(
    FILES
      "${CMAKE_CURRENT_BINARY_DIR}/bare_kit.lib"
      "${CMAKE_CURRENT_BINARY_DIR}/bare_kit_deps.lib"
      "${CMAKE_CURRENT_BINARY_DIR}/bare_kit.libs"
    DESTINATION ${target}
  )
else()
  set_target_properties(
    bare_kit_prelink
    PROPERTIES
    OUTPUT_NAME bare-kit
    SUFFIX .o
  )

  target_link_options(
    bare_kit_prelink
    PRIVATE
      -r
      -nostdlib
  )

  set(archive "${CMAKE_CURRENT_BINARY_DIR}/libbare_kit.a")

  add_custom_command(
    OUTPUT "${archive}"
    COMMAND "${CMAKE_COMMAND}" -E rm -f "${archive}"
    COMMAND "${CMAKE_AR}" rcs "${archive}" "$<TARGET_FILE:bare_kit_prelink>"
    DEPENDS bare_kit_prelink
  )

  add_custom_target(bare_kit_rust ALL DEPENDS "${archive}")

  install(FILES "${archive}" DESTINATION ${target})
endif()
