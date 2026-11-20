// K7Embed.h — the C calls of the Rust front's library (apps/k7-daw-backend/packages/k7-embed), linked as libk7_embed.a.
// A start answers the port once it is bound, or a negative code (apps/k7-daw-backend/docs/embed.md).
// The roaming list's store is two callbacks installed before the start (apps/k7-daw-backend/docs/roam.md).

#include <stdint.h>

int32_t k7_front_start(const char *config_json);
int32_t k7_front_stop(void);

typedef void (*k7_roam_emit)(void *ctx, const char *key, const char *value);
typedef void (*k7_roam_list)(k7_roam_emit emit, void *ctx);
typedef void (*k7_roam_put)(const char *key, const char *value);
void k7_roam_install(k7_roam_list list, k7_roam_put put);
