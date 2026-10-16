# DLLs (LGPL: separately replaceable) but with the C/C++ runtime linked statically,
# so the installer does not depend on the Visual C++ Redistributable being present.
set(VCPKG_TARGET_ARCHITECTURE x64)
set(VCPKG_CRT_LINKAGE static)
set(VCPKG_LIBRARY_LINKAGE dynamic)
set(VCPKG_BUILD_TYPE release)
