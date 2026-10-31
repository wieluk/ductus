// Include guards: the build reaches headers through two include paths.
#ifndef DUCTUS_APP_H
#define DUCTUS_APP_H

#include <cstdint>

#include "rust/cxx.h"

std::int32_t run_app(rust::Str program, rust::Vec<rust::String> files);
/// Drawing with Qt's software renderer instead of the graphics card (applies at start).
bool softwareRendering();

#endif
