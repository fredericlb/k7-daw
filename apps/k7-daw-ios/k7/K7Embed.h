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

// Discovery of other k7 devices is the host's: the advertiser is a callback installed before the start, the browse result is pushed (apps/k7-daw-backend/docs/lan.md).
typedef void (*k7_lan_advertise)(const char *record_json);
void k7_lan_install(k7_lan_advertise advertise);
int32_t k7_lan_peers(const char *peers_json);
void k7_lan_set_open(int32_t open);

// The supervisor, run on a thread of the app over two pipes: the NDJSON a desktop app exchanges with it over stdio (apps/k7-daw-backend/packages/k7-embed/src/supervisor.rs).
int32_t k7_supervisor_run(int32_t commands_fd, int32_t frames_fd);
int32_t k7_supervisor_cuts_in_flight(void);
void k7_supervisor_log(const char *tag, const char *text);
