#!/bin/bash
# The libraries built in MSYS2's UCRT64 shell for the installer, as VPX 5436's build makes them: FFmpeg as its own script
# configures it (platforms/windows-x64/external.sh: shared, no programs or documents, LGPL — no
# GPL or non-free parts — its names ending in 64), avdevice and avfilter left out, which the player
# never loads; and the USB libraries libdof and libdmdutil talk to cabinets through, as libdof's own
# script builds them at the versions it pins (platforms/win/x64/external.sh). 8 jobs; the caller
# runs it at low priority (setup.ps1). Each built once: a library already built is only collected.
#
#   MSYSTEM=UCRT64 bash -l build-mingw.sh <vpx-build/thirdparty, as /c/...>

set -e

ROOT="$1"
SRC="$ROOT/src"
OUT="$ROOT/out/windows-x64"
JOBS=8
mkdir -p "$SRC" "$OUT/bin" "$OUT/lib" "$OUT/include"

# a pinned source, unpacked once into its own folder
fetch() {
   local name="$1" url="$2"
   if [ ! -d "$SRC/$name" ]; then
      echo "Fetching $name ..."
      curl -sSL "$url" -o "$SRC/$name.tar.gz"
      rm -rf "$SRC/$name.tmp" && mkdir -p "$SRC/$name.tmp"
      tar xzf "$SRC/$name.tar.gz" -C "$SRC/$name.tmp"
      mv "$SRC/$name.tmp/"* "$SRC/$name"
      rmdir "$SRC/$name.tmp"
   fi
}

# libusb (LGPL-2.1-or-later), named as libdof's script names it
fetch libusb https://github.com/libusb/libusb/archive/15a7ebb4d426c5ce196684347d2b7cafad862626.tar.gz
cd "$SRC/libusb"
if [ ! -f libusb/.libs/libusb64-1.0.dll ]; then
   echo "Building libusb ..."
   sed -i 's/libusb-1\.0/libusb64-1.0/g; s/libusb_1_0/libusb64_1_0/g' libusb/Makefile.am
   [ -f libusb/libusb-1.0.def ] && mv libusb/libusb-1.0.def libusb/libusb64-1.0.def
   [ -f libusb/libusb-1.0.rc ] && mv libusb/libusb-1.0.rc libusb/libusb64-1.0.rc
   sed -i 's/libusb-1\.0/libusb64-1.0/g' libusb/libusb64-1.0.def libusb/libusb64-1.0.rc
   NOCONFIGURE=1 ./autogen.sh
   ./configure --enable-shared
   make -j$JOBS
fi
mkdir -p "$OUT/include/libusb-1.0"
cp libusb/libusb.h "$OUT/include/libusb-1.0/"
cp libusb/.libs/libusb64-1.0.dll.a "$OUT/lib/libusb64-1.0.lib"
cp libusb/.libs/libusb64-1.0.dll "$OUT/bin/"

# libserialport (LGPL-3.0-or-later)
fetch libserialport https://github.com/sigrokproject/libserialport/archive/21b3dfe5f68c205be4086469335fd2fc2ce11ed2.tar.gz
cd "$SRC/libserialport"
if [ ! -f .libs/libserialport64-0.dll ]; then
   echo "Building libserialport ..."
   sed -i 's/libserialport\.la/libserialport64.la/g; s/libserialport_la/libserialport64_la/g' Makefile.am
   ./autogen.sh
   ./configure
   make -j$JOBS
fi
cp libserialport.h "$OUT/include/"
cp .libs/libserialport64.dll.a "$OUT/lib/libserialport64.lib"
cp .libs/libserialport64-0.dll "$OUT/bin/"

# hidapi (BSD-3-Clause, among its three licences)
fetch hidapi https://github.com/libusb/hidapi/archive/d6b2a974608dec3b76fb1e36c189f22b9cf3650c.tar.gz
cd "$SRC/hidapi"
if [ ! -f build/src/windows/hidapi64.dll ]; then
   echo "Building hidapi ..."
   grep -q 'OUTPUT_NAME "hidapi64"' windows/CMakeLists.txt || sed -i 's/OUTPUT_NAME "hidapi"/OUTPUT_NAME "hidapi64"\n  PREFIX ""\n  IMPORT_PREFIX ""/' windows/CMakeLists.txt
   cmake -G Ninja -DCMAKE_BUILD_TYPE=Release -B build
   cmake --build build -j $JOBS
fi
cp -r hidapi "$OUT/include/"
cp build/src/windows/hidapi64.dll.a "$OUT/lib/hidapi64.lib"
cp build/src/windows/hidapi64.dll "$OUT/bin/"

# libftdi (LGPL-2.1-only for the library), the fork libdof pins, against our libusb
fetch libftdi https://github.com/jsm174/libftdi/archive/5c2c58e03ea999534e8cb64906c8ae8b15536c30.tar.gz
cd "$SRC/libftdi"
if [ ! -f build/src/libftdi164.dll ]; then
   echo "Building libftdi ..."
   sed -i 's/cmake_minimum_required([^)]*)/cmake_minimum_required(VERSION 3.10)/' CMakeLists.txt
   grep -q 'OUTPUT_NAME "ftdi164"' src/CMakeLists.txt || sed -i 's/set_target_properties(ftdi1 PROPERTIES VERSION ${VERSION_FIXUP}\.${MINOR_VERSION}\.0 SOVERSION 2)/set_target_properties(ftdi1 PROPERTIES OUTPUT_NAME "ftdi164" VERSION ${VERSION_FIXUP}.${MINOR_VERSION}.0 SOVERSION 2)/' src/CMakeLists.txt
   cmake -G Ninja -DFTDI_EEPROM=OFF -DEXAMPLES=OFF -DSTATICLIBS=OFF \
      -DLIBUSB_INCLUDE_DIR="$SRC/libusb/libusb" -DLIBUSB_LIBRARIES="$OUT/bin/libusb64-1.0.dll" \
      -DCMAKE_BUILD_TYPE=Release -B build
   cmake --build build -j $JOBS
fi
mkdir -p "$OUT/include/libftdi1"
cp src/ftdi.h "$OUT/include/libftdi1/"
cp build/src/libftdi164.dll.a "$OUT/lib/libftdi164.lib"
cp build/src/libftdi164.dll "$OUT/bin/"

# FFmpeg (LGPL-2.1-or-later), VPX 5436's commit and configuration
fetch ffmpeg https://github.com/FFmpeg/FFmpeg/archive/38b88335f99e76ed89ff3c93f877fdefce736c13.tar.gz
cd "$SRC/ffmpeg"
if ! ls libavcodec/avcodec64-*.dll >/dev/null 2>&1; then
   echo "Building FFmpeg (about a quarter of an hour) ..."
   ./configure --enable-shared --disable-static --disable-programs --disable-doc --disable-avdevice --disable-avfilter \
      --arch=x86_64 --build-suffix=64
   make -j$JOBS
fi
for lib in avutil avcodec avformat swscale swresample; do
   cp lib$lib/${lib}64-*.dll "$OUT/bin/"
   # its import library: made by FFmpeg as a .lib when it can, else MinGW's own kind
   cp lib$lib/${lib}64.lib "$OUT/lib/" 2>/dev/null || cp lib$lib/${lib}64.dll.a "$OUT/lib/${lib}64.lib" 2>/dev/null || cp lib$lib/lib${lib}64.dll.a "$OUT/lib/${lib}64.lib"
   mkdir -p "$OUT/include/lib$lib"
   cp lib$lib/*.h "$OUT/include/lib$lib/"
done

# the MinGW runtime and the small libraries they load, from MSYS2's UCRT64 (each its own licence:
# GCC's runtime exception, winpthreads' MIT and ZPL, zlib's, bzip2's, xz's 0BSD, libiconv's LGPL)
for dll in libgcc_s_seh-1 libstdc++-6 libwinpthread-1 zlib1 libbz2-1 liblzma-5 libiconv-2; do
   [ -f "/ucrt64/bin/$dll.dll" ] && cp "/ucrt64/bin/$dll.dll" "$OUT/bin/"
done
echo "MinGW libraries done: $OUT"
