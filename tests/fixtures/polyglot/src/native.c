#include "native.h"

int c_helper(int value) {
    return value + 1;
}

int c_login(int value) {
    return c_helper(value);
}
