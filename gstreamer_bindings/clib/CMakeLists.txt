cmake_minimum_required(VERSION "3.28.3")

project(frame_extractor)
set(CMAKE_CXX_STANDARD 23)


find_package(PkgConfig REQUIRED)
pkg_check_modules(gtk3 REQUIRED IMPORTED_TARGET gtk+-3.0)
pkg_search_module(gstreamer REQUIRED IMPORTED_TARGET gstreamer-1.0>=1.10)
pkg_search_module(gstreamer-pbutils REQUIRED IMPORTED_TARGET gstreamer-pbutils-1.0>=1.10)
pkg_search_module(gstreamer-app REQUIRED IMPORTED_TARGET gstreamer-app-1.0>=1.10)
pkg_search_module(gstreamer-video REQUIRED IMPORTED_TARGET gstreamer-video-1.0>=1.10)

add_library(frame_extractor STATIC 
  "src/frame_extractor.cpp"
)

target_include_directories(frame_extractor PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include/")

target_link_libraries(frame_extractor
  PkgConfig::gtk3
  PkgConfig::gstreamer
  PkgConfig::gstreamer-pbutils
  PkgConfig::gstreamer-app
  PkgConfig::gstreamer-video
)
