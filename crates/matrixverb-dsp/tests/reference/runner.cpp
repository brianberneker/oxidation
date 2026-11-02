#include "MatrixVerb.h"
#include <algorithm>
#include <array>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>

constexpr int block = 256;
constexpr int frames = 32768;

void parameters(MatrixVerb &core, int scenario, int position) {
    const float cases[][7] = {
        {1, 0, 0, 0, .5f, .5f, 1},
        {0, 0, 0, 0, 0, 0, 0},
        {1, 1, 1, 1, 1, 1, 1},
        {.2f, .6f, .9f, 1, 0, .5f, .4f},
        {.83f, .13f, .71f, .53f, .37f, .13f, .71f},
        {.83f, .13f, .71f, .53f, .37f, .91f, .71f},
        {1, 1, .2f, .5f, .2f, .5f, 1},
        {.1f, .5f, .5f, .5f, .1f, .5f, 0},
    };
    int index = scenario < 8 ? scenario : 0;
    if (scenario == 9) index = position < 8192 ? 2 : (position < 16384 ? 3 : 4);
    const auto &p = cases[index];
    core.A = p[0]; core.B = p[1]; core.C = p[2]; core.D = p[3];
    core.E = p[4]; core.F = p[5]; core.G = p[6];
}

float input(int frame, int channel, int scenario) {
    if (scenario == 8) return 0.0f;
    if (frame == 0) return channel == 0 ? 1.0f : -0.5f;
    if (frame >= 10000) return 0.0f;
    return float((frame * (channel == 0 ? 17 : 29)) % 97 - 48) * 0.0001f;
}

template<typename T>
void write(T value) {
    using Bits = typename std::conditional<sizeof(T) == 4, uint32_t, uint64_t>::type;
    Bits bits;
    std::memcpy(&bits, &value, sizeof(T));
    for (size_t byte = 0; byte < sizeof(T); ++byte) std::putchar((bits >> (byte * 8)) & 255);
}

template<typename T>
void process(MatrixVerb &core, T **in, T **out) {
    if constexpr (sizeof(T) == 4) core.processReplacing(in, out, block);
    else core.processDoubleReplacing(in, out, block);
}

template<typename T>
void render(double rate, int scenario) {
    auto core = std::make_unique<MatrixVerb>();
    core->sampleRate = rate;
    T left[block], right[block], outL[block], outR[block];
    T *in[] = {left, right}, *out[] = {outL, outR};
    for (int position = 0; position < frames; position += block) {
        parameters(*core, scenario, position);
        for (int i = 0; i < block; ++i) {
            left[i] = input(position + i, 0, scenario);
            right[i] = input(position + i, 1, scenario);
        }
        process(*core, in, out);
        for (int i = 0; i < block; ++i) { write(outL[i]); write(outR[i]); }
    }
}

void benchmark() {
    auto core = std::make_unique<MatrixVerb>();
    float left[block], right[block], outL[block], outR[block];
    float *in[] = {left, right}, *out[] = {outL, outR};
    for (int i = 0; i < block; ++i) {
        left[i] = input(i + 1, 0, 0);
        right[i] = input(i + 1, 1, 0);
    }
    constexpr int iterations = (1 << 17) / block;
    for (int i = 0; i < iterations; ++i) core->processReplacing(in, out, block);
    std::array<double, 11> times;
    for (double &time : times) {
        auto start = std::chrono::steady_clock::now();
        for (int i = 0; i < iterations; ++i) core->processReplacing(in, out, block);
        auto elapsed = std::chrono::steady_clock::now() - start;
        time = std::chrono::duration<double, std::nano>(elapsed).count() / (iterations * block);
    }
    std::sort(times.begin(), times.end());
    std::printf("Original C++ MatrixVerb: median %.3f ns/stereo frame, min %.3f, max %.3f\n",
                times[5], times.front(), times.back());
}

int main(int argc, char **argv) {
    if (argc == 2 && std::strcmp(argv[1], "bench") == 0) { benchmark(); return 0; }
    if (argc != 4) {
        std::fprintf(stderr, "usage: matrixverb-reference bench | RATE SCENARIO f32|f64\n");
        return 1;
    }
    if (std::strcmp(argv[3], "f32") == 0) render<float>(std::atof(argv[1]), std::atoi(argv[2]));
    else if (std::strcmp(argv[3], "f64") == 0) render<double>(std::atof(argv[1]), std::atoi(argv[2]));
    else return 1;
}
