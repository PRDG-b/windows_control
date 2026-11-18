/* Read-only capture diagnosis: inspect mmap bytes immediately after DQBUF,
 * before any file write or QBUF. Does not change capture format or send HID.
 * cc -O2 -Wall -Wextra v4l2_buffer_probe.c -o v4l2_buffer_probe
 * ./v4l2_buffer_probe /dev/video0 <existing-output-directory> [frame-count]
 */
#include <errno.h>
#include <fcntl.h>
#include <linux/videodev2.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <unistd.h>

static void fail(const char *operation) { perror(operation); exit(1); }

int main(int argc, char **argv) {
    if (argc < 3 || argc > 4) return 2;
    unsigned count = argc == 4 ? (unsigned)atoi(argv[3]) : 90;
    if (!count || count > 1000) return 2;
    int fd = open(argv[1], O_RDWR | O_NONBLOCK);
    if (fd < 0) fail("open");
    struct v4l2_format format = {.type = V4L2_BUF_TYPE_VIDEO_CAPTURE};
    if (ioctl(fd, VIDIOC_G_FMT, &format) < 0) fail("G_FMT");
    if (format.fmt.pix.pixelformat != V4L2_PIX_FMT_MJPEG) {
        fprintf(stderr, "MJPEG capture required; no format change performed\n");
        close(fd); return 2;
    }
    struct v4l2_requestbuffers request = {.count = 4, .type = V4L2_BUF_TYPE_VIDEO_CAPTURE, .memory = V4L2_MEMORY_MMAP};
    if (ioctl(fd, VIDIOC_REQBUFS, &request) < 0) fail("REQBUFS");
    if (!request.count || request.count > 32) { close(fd); return 2; }
    void *memory[32] = {0}; size_t lengths[32] = {0};
    for (unsigned i = 0; i < request.count; i++) {
        struct v4l2_buffer buffer = {.type = request.type, .memory = request.memory, .index = i};
        if (ioctl(fd, VIDIOC_QUERYBUF, &buffer) < 0) fail("QUERYBUF");
        lengths[i] = buffer.length;
        memory[i] = mmap(NULL, buffer.length, PROT_READ | PROT_WRITE, MAP_SHARED, fd, buffer.m.offset);
        if (memory[i] == MAP_FAILED) fail("mmap");
        if (ioctl(fd, VIDIOC_QBUF, &buffer) < 0) fail("QBUF");
    }
    enum v4l2_buf_type type = request.type;
    if (ioctl(fd, VIDIOC_STREAMON, &type) < 0) fail("STREAMON");
    puts("frame,sequence,timestamp,bytesused,flags,error,soi,eoi,tail");
    unsigned bad = 0;
    for (unsigned i = 0; i < count;) {
        struct pollfd ready = {.fd = fd, .events = POLLIN};
        int result = poll(&ready, 1, 2000);
        if (result <= 0) { fprintf(stderr, "poll timeout/error\n"); close(fd); return 1; }
        struct v4l2_buffer buffer = {.type = type, .memory = request.memory};
        if (ioctl(fd, VIDIOC_DQBUF, &buffer) < 0) { if (errno == EAGAIN) continue; fail("DQBUF"); }
        if (buffer.index >= request.count || buffer.bytesused > lengths[buffer.index]) { close(fd); return 1; }
        unsigned char *p = memory[buffer.index]; unsigned n = buffer.bytesused;
        int soi = n >= 2 && p[0] == 0xff && p[1] == 0xd8;
        int eoi = n >= 2 && p[n-2] == 0xff && p[n-1] == 0xd9;
        unsigned tail = n >= 2 ? (unsigned)p[n-2] * 256 + p[n-1] : 0;
        printf("%u,%u,%ld.%06ld,%u,0x%x,%d,%d,%d,%04x\n", i, buffer.sequence,
               (long)buffer.timestamp.tv_sec, (long)buffer.timestamp.tv_usec,
               n, buffer.flags, !!(buffer.flags & V4L2_BUF_FLAG_ERROR), soi, eoi, tail);
        if (!soi || !eoi) {
            bad++;
            char path[4096];
            if (snprintf(path, sizeof(path), "%s/mmap-bad-%03u.jpg", argv[2], i) >= (int)sizeof(path)) return 2;
            FILE *output = fopen(path, "wbx");
            if (!output) fail("create artifact");
            if (fwrite(p, 1, n, output) != n) fail("write artifact");
            if (fclose(output)) fail("close artifact");
        }
        if (ioctl(fd, VIDIOC_QBUF, &buffer) < 0) fail("QBUF");
        i++;
    }
    if (ioctl(fd, VIDIOC_STREAMOFF, &type) < 0) fail("STREAMOFF");
    for (unsigned i = 0; i < request.count; i++) munmap(memory[i], lengths[i]);
    close(fd);
    fprintf(stderr, "frames=%u bad_markers=%u\n", count, bad);
    return 0;
}
