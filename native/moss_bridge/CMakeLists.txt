cmake_minimum_required(VERSION 3.20)

project(MossRSBridge LANGUAGES CXX)

add_library(moss_bridge STATIC
    src/core.cpp
)

target_include_directories(moss_bridge
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_compile_features(moss_bridge PRIVATE cxx_std_17)

if(MOSS_DIR)
    target_include_directories(moss_bridge PRIVATE
        ${MOSS_DIR}/include
        ${MOSS_DIR}/src
    )
endif()
