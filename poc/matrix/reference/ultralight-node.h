// DSP-only shim for extracted, unchanged MIT Airwindows Ultralight3 expressions.
// Copyright (c) 2026 airwindows. See ../LICENSE.airwindows.
#pragma once
#include <array>
#include <cstddef>

struct UltralightNode {
#define LINE(N, LENGTH) \
    static constexpr int d4##N = LENGTH; \
    float a##N[LENGTH + 5] = {}; \
    int c##N = 1;
    LINE(A, 1626) LINE(B, 650) LINE(C, 470) LINE(D, 47)
    LINE(E, 1149) LINE(F, 575) LINE(G, 456) LINE(H, 52)
    LINE(I, 1756) LINE(J, 888) LINE(K, 173) LINE(L, 49)
    LINE(M, 1798) LINE(N, 776) LINE(O, 152) LINE(P, 21)
#undef LINE
    float fA = 0, fB = 0, fC = 0, fD = 0;
    float fhA = 0, fhB = 0, fhC = 0, fhD = 0;
    float flA = 0, flB = 0, flC = 0, flD = 0;
    float hA = 0, hB = 0, hC = 0, hD = 0, hE = 0, hF = 0, hG = 0, hH = 0, hI = 0;

    void process(const float *input, std::array<float, 8> *output,
                 std::size_t count, int step, int unSolid, int unReflect);
};
