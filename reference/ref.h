
struct Vector3 {
    float x;
    float y;
    float z;
};

/// @pattern 40 53 48 81 EC ? 00 00 00 48 C7 44 24 24 30
typedef void testing1(struct Vector3* a);

/// @pattern FE FF FF FF 48 8B DA ? ? D1 48 8D 4C 24 30 E8 7E BD 11 00
typedef void testing2(int a, long b);
