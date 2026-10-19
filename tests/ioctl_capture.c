/* Test-only interception in a subprocess using /dev/null; never touches USB. */
#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <errno.h>
#include <string.h>
int ioctl(int fd, unsigned long request, ...) {
    (void)fd;
    if (request != 0xc0084806UL && request != 0xc0084807UL) { errno = 25; return -1; }
    va_list args; va_start(args, request);
    unsigned char *buf = va_arg(args, unsigned char *); va_end(args);
    if (request == 0xc0084807UL) {
        unsigned char response[8] = {7, 1, 0x16, 0, 0, 0, 0, 0};
        const char *fixture = getenv("LECOO_TEST_RESPONSE");
        if (fixture && strcmp(fixture, "bad-id") == 0) response[0] = 9;
        if (fixture && strcmp(fixture, "empty") == 0) memset(response + 1, 0, 7);
        if (fixture && strcmp(fixture, "bad-version") == 0) response[2] = 0xfa;
        memcpy(buf, response, 8);
        if (fixture && strcmp(fixture, "short") == 0) return 3;
        return 8;
    }
    const char *path = getenv("LECOO_TEST_TRACE");
    FILE *out = path ? fopen(path, "a") : NULL;
    if (!out) return -1;
    for (int i=0;i<8;i++) fprintf(out,"%02X%s",buf[i],i==7?"\n":" ");
    fclose(out);
    return getenv("LECOO_TEST_SHORT_SET") ? 3 : 8;
}
