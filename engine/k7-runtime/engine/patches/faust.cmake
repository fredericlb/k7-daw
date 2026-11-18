# faust.cmake — the fix Faust's poly-dsp.h needs, applied at fetch time (PATCH_COMMAND), like the eurorack scripts.
# The one thing a reader would get wrong: these prints are not debug-only upstream, they run on EVERY voice steal.

include("${CMAKE_CURRENT_LIST_DIR}/../eurorack/patches/studio_replace.cmake")

# keyOn/keyOff run on the audio thread, and each print takes the FILE lock and a write(2) — a stall, or a hang on an undrained pipe (docs/faust.md).
set(poly "architecture/faust/dsp/poly-dsp.h")
studio_replace("${poly}" "fprintf(stderr, \"Steal release voice" "if (false) fprintf (stderr, \"Steal release voice")
studio_replace("${poly}" "fprintf(stderr, \"Steal playing voice" "if (false) fprintf (stderr, \"Steal playing voice")
studio_replace("${poly}" "fprintf(stderr, \"DSP is not polyphonic" "if (false) fprintf (stderr, \"DSP is not polyphonic")
studio_replace("${poly}" "fprintf(stderr, \"Playing pitch = %d not found" "if (false) fprintf (stderr, \"Playing pitch = %d not found")

# libc++ orders a std::set<T*> by raw pointer even when std::less<T*> is specialised, so Faust's serial ordering splits one set in two (docs/faust.md).
set(tree "compiler/tlib/tree.hh")
set(less_decl [=[template <>
struct less<CTree*> {
    bool operator()(const CTree* lhs, const CTree* rhs) const;
};]=])
set(less_def [=[inline bool less<CTree*>::operator()(const CTree* lhs, const CTree* rhs) const
{
    return lhs->serial() < rhs->serial();
}]=])
# A kept source an earlier revision of this script patched holds comments where the blocks were: they take the guarded blocks.
file(READ "${tree}" tree_text)
string(FIND "${tree_text}" "// std::less<CTree*> is left generic on Apple" old_at)
if(NOT old_at EQUAL -1)
    studio_replace("${tree}" "// std::less<CTree*> is left generic on Apple: a specialisation there is half ignored." "#if !defined(_LIBCPP_VERSION)\n${less_decl}\n#endif")
    studio_replace("${tree}" "// (the serial-ordered std::less<CTree*> definition is removed on Apple)" "#if !defined(_LIBCPP_VERSION)\n${less_def}\n#endif")
endif()
# The guard is the C++ library, decided per target at compile time: one fetched source serves every target of a host.
studio_replace("${tree}" "${less_decl}" "#if !defined(_LIBCPP_VERSION)\n${less_decl}\n#endif")
studio_replace("${tree}" "${less_def}" "#if !defined(_LIBCPP_VERSION)\n${less_def}\n#endif")

# Faust pins an iOS build to the device SDK and three slices, which Ninja splits into stray file names and which overrides the preset's simulator sysroot (docs/faust.md).
set(top "build/CMakeLists.txt")
studio_replace("${top}" "        set (CMAKE_XCODE_EFFECTIVE_PLATFORMS \"iphoneos\")\n        set (CMAKE_OSX_ARCHITECTURES \"arm64 armv7 armv7s\")\n        set (CMAKE_OSX_SYSROOT \"iphoneos\")\n" "        # the preset sets the iOS platform, slice and sysroot\n")
