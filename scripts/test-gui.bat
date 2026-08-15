@echo off
set "PATH=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;D:\tools\Android\sdk\ndk\29.0.14033849\toolchains\llvm\prebuilt\windows-x86_64\bin;%PATH%"
set "LIBCLANG_PATH=D:\tools\Android\sdk\ndk\29.0.14033849\toolchains\llvm\prebuilt\windows-x86_64\bin"
set "BINDGEN_EXTRA_CLANG_ARGS=--target=x86_64-pc-windows-msvc -I"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt" -I"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\include" -I"C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\um""

cargo test -p operon-gui
