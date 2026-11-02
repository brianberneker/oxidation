// MatrixVerb DSP state shim for the pinned, unmodified MIT Airwindows processor.
// Copyright (c) 2016 airwindows. See poc/matrix/LICENSE.airwindows.
#ifndef __MatrixVerb_H
#define __MatrixVerb_H

#include <cmath>
#include <cstdint>

using VstInt32 = int32_t;

class MatrixVerb {
public:
    double sampleRate = 44100.0;
    float A = 1.0f, B = 0.0f, C = 0.0f, D = 0.0f, E = 0.5f, F = 0.5f, G = 1.0f;
    uint32_t fpdL = 0x12345678, fpdR = 0x87654321;
    double biquadA[11] = {}, biquadB[11] = {}, biquadC[11] = {};

#define MATRIX_LINE(N, CAPACITY, RATE, LEFT, RIGHT) \
    double a##N##L[CAPACITY] = {}, a##N##R[CAPACITY] = {}; \
    int count##N = 1, delay##N = 1; \
    double feedback##N##L = 0.0, feedback##N##R = 0.0; \
    double vib##N##L = LEFT, vib##N##R = RIGHT, depth##N = RATE;
    MATRIX_LINE(A, 8111, 0.003251, 0.17, 1.91)
    MATRIX_LINE(B, 7511, 0.002999, 0.83, 2.71)
    MATRIX_LINE(C, 7311, 0.002917, 1.47, 3.37)
    MATRIX_LINE(D, 6911, 0.002749, 2.13, 4.03)
    MATRIX_LINE(E, 6311, 0.002503, 2.79, 4.69)
    MATRIX_LINE(F, 6111, 0.002423, 3.43, 5.33)
    MATRIX_LINE(G, 5511, 0.002146, 4.09, 5.99)
    MATRIX_LINE(H, 4911, 0.002088, 4.73, 0.37)
#undef MATRIX_LINE

#define INPUT_LINE(N, CAPACITY) \
    double a##N##L[CAPACITY] = {}, a##N##R[CAPACITY] = {}; \
    int count##N = 1, delay##N = 1;
    INPUT_LINE(I, 4511)
    INPUT_LINE(J, 4311)
    INPUT_LINE(K, 3911)
    INPUT_LINE(L, 3311)
    INPUT_LINE(M, 3111)
#undef INPUT_LINE

    double getSampleRate() const { return sampleRate; }
    void processReplacing(float **inputs, float **outputs, VstInt32 sampleFrames);
    void processDoubleReplacing(double **inputs, double **outputs, VstInt32 sampleFrames);
};
#endif
