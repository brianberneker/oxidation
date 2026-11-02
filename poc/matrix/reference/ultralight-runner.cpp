#include "ultralight-node.h"
#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>

template<typename T>
void write(T value) {
    uint32_t bits;
    static_assert(sizeof(value) == sizeof(bits));
    std::memcpy(&bits, &value, sizeof(value));
    for (int byte = 0; byte < 4; ++byte) std::putchar((bits >> (byte * 8)) & 255);
}

void corpus() {
    std::fwrite("AWNODE01", 1, 8, stdout);
    write(uint32_t(75));
    uint32_t seed = 0x7abeca01;
    for (int step : {1, 5, 9}) {
        for (int high = 0; high <= 4; ++high) {
            for (int low = 0; low <= 4; ++low) {
                auto node = std::make_unique<UltralightNode>();
                int count = step == 1 && high == 0 && low == 0 ? 480000 : 20000;
                std::putchar(step); std::putchar(high); std::putchar(low); std::putchar(0);
                write(uint32_t(count));
                for (int frame = 0; frame < count; ++frame) {
                    seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
                    float input = frame == 0 ? 1.0f :
                        (frame < 10000 && high != low ? float(int32_t(seed)) * (0.01f / float(INT32_MAX)) : 0.0f);
                    std::array<float, 8> output;
                    node->process(&input, &output, 1, step, high, low);
                    write(input);
                    for (float sample : output) write(sample);
                }
            }
        }
    }
}

void benchmark() {
    auto node = std::make_unique<UltralightNode>();
    constexpr int block = 256, iterations = (1 << 19) / block;
    float input[block];
    std::array<float, 8> output[block];
    for (int frame = 0; frame < block; ++frame) input[frame] = (float(frame * 17 % 97) - 48.0f) * 0.0001f;
    for (int i = 0; i < iterations; ++i) node->process(input, output, block, 5, 0, 0);
    std::array<double, 11> times;
    for (double &time : times) {
        auto start = std::chrono::steady_clock::now();
        for (int i = 0; i < iterations; ++i) node->process(input, output, block, 5, 0, 0);
        time = std::chrono::duration<double, std::nano>(std::chrono::steady_clock::now() - start).count() / (iterations * block);
    }
    std::sort(times.begin(), times.end());
    std::printf("Original C++ Ultralight3 core: median %.3f ns/sample, min %.3f, max %.3f\n",
                times[5], times.front(), times.back());
}

int main(int argc, char **argv) {
    if (argc == 2 && std::strcmp(argv[1], "corpus") == 0) { corpus(); return 0; }
    if (argc == 2 && std::strcmp(argv[1], "bench") == 0) { benchmark(); return 0; }
    std::fprintf(stderr, "usage: ultralight-reference corpus | bench\n");
    return 1;
}
