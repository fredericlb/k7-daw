# studio_replace.cmake — the one edit the fetch-time patch scripts make, shared by eurorack.cmake, stmlib.cmake
# and engine/patches/{faust,tracktion,surge,sfizz,ixwebsocket}.cmake. It fails LOUDLY when its text is not found: a bump that moved the code must be looked at.
#
# And it is a no-op when the edit is already there. The patch step runs AGAIN on a source it has
# already patched whenever its command changes, and the command names this script by an absolute
# path — which, on CI, holds the build's own number. Every CI run re-patches
# the kept sources, so `to` must be recognisable: never a bare token that the file holds anyway.
# The edit is looked for FIRST: a `to` that still holds its `from` would otherwise be applied once per run.

function(studio_replace file from to)
    file(READ "${file}" text)
    string(FIND "${text}" "${to}" done)
    if(NOT done EQUAL -1)
        return()
    endif()
    string(FIND "${text}" "${from}" at)
    if(at EQUAL -1)
        message(FATAL_ERROR "fetch-time patch: text not found in ${file} — was the source bumped?")
    endif()
    string(REPLACE "${from}" "${to}" text "${text}")
    file(WRITE "${file}" "${text}")
endfunction()
