// This file is generated automatically. To update it forcefully, run `cargo run -p warren-ios --target aarch64-apple-ios`.

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Used by Swift to instruct which access method kind it is trying to convert
 */
enum SwiftAccessMethodKind {
  KindDirect = 0,
  KindBridge,
  KindEncryptedDnsProxy,
  KindShadowsocks,
  KindSocks5Local,
  KindDomainFronting,
};
typedef uint8_t SwiftAccessMethodKind;

/**
 * Event tag for the variant union below. Variants are constructed by
 * the warren-tunnel dispatcher once the Quinn connection task is
 * wired.
 */
typedef enum WarrenTunnelEventTagC {
  EventConnected = 0,
  EventDisconnected = 1,
  EventReconnecting = 2,
  EventFailover = 3,
  EventNatPmpMapped = 4,
  EventNatPmpRenewed = 5,
  EventNatPmpFailed = 6,
  /**
   * Fired once, on the very first connection attempt of a tunnel
   * session (before any successful `Connected` event). Subsequent
   * attempts after a connection drop fire `EventReconnecting`
   * instead. Swift uses this to distinguish "tunnel starting" from
   * "tunnel recovering".
   */
  EventConnecting = 7,
} WarrenTunnelEventTagC;

/**
 * Tunnel state enum surfaced via `warren_tunnel_status`. Variants
 * other than `Disconnected` are constructed by the warren-tunnel
 * dispatcher, which only compiles on the iOS target with the `tunnel`
 * feature. Off that exact path - including every host (clippy / test)
 * build, with or without `tunnel` - the enum has no constructor, hence
 * the `cfg_attr(expect)` suppression scoped to `not(ios && tunnel)`.
 */
typedef enum WarrenTunnelStateC {
  Disconnected = 0,
  Connecting = 1,
  Connected = 2,
  Reconnecting = 3,
  Failed = 4,
} WarrenTunnelStateC;

typedef struct ApiContext ApiContext;

typedef struct DomainFrontingConfigContext DomainFrontingConfigContext;

typedef struct RequestCancelHandle RequestCancelHandle;

typedef struct RetryStrategy RetryStrategy;

typedef struct SwiftAccessMethodSettingsContext SwiftAccessMethodSettingsContext;

/**
 * Opaque handle representing an active Warren tunnel. Created by
 * [`warren_tunnel_start`] ; destroyed by [`warren_tunnel_stop`].
 * The Swift side treats this as an `OpaquePointer`.
 */
typedef struct WarrenTunnelHandle {
  uint8_t _private[0];
} WarrenTunnelHandle;

/**
 * Multi-hop entry relay configuration.
 */
typedef struct WarrenRelayConfigC {
  /**
   * 32-byte Ed25519 public key of the entry relay.
   */
  uint8_t pubkey[32];
  /**
   * Null-terminated UTF-8 "IP:port" of the entry relay.
   */
  const char *endpoint;
  /**
   * Null-terminated UTF-8 ISO 3166-1 alpha-2 country code.
   */
  const char *country_code;
} WarrenRelayConfigC;

/**
 * DAITA defensive shaping spec.
 */
typedef struct WarrenDaitaSpecC {
  /**
   * 32-byte Maybenot machine seed.
   */
  uint8_t machine_seed[32];
  /**
   * Padding budget in packets/sec.
   */
  uint32_t padding_pps;
} WarrenDaitaSpecC;

/**
 * Parameters passed from Swift to start a Warren tunnel. Mirrors the
 * public surface of `warren_tunnel::WarrenTunnelParameters`.
 *
 * String fields are null-terminated UTF-8 ; the Swift side allocates
 * and retains them for the duration of the `warren_tunnel_start`
 * call (Rust only borrows during marshalling).
 */
typedef struct WarrenTunnelParametersC {
  /**
   * 32-byte Ed25519 public key of the exit relay.
   */
  uint8_t exit_pubkey[32];
  /**
   * Null-terminated UTF-8 "IP:port" of the exit relay.
   */
  const char *exit_endpoint;
  /**
   * 32-byte Ed25519 signing seed derived from the user wallet
   * (see `warren_wallet_seed_from_mnemonic` + `derive_node_key`).
   */
  uint8_t wallet_signing_seed[32];
  /**
   * Optional multi-hop entry relay. Superseded by directory-driven
   * selection (the entry relay is chosen from `multihop_directory_json`),
   * so this field is no longer consumed.
   */
  const struct WarrenRelayConfigC *multi_hop_relay;
  /**
   * Optional DAITA defensive shaping spec. Null when DAITA OFF.
   */
  const struct WarrenDaitaSpecC *daita_spec;
  /**
   * 1 enables NAT-PMP port forwarding through the tunnel ; 0 disables.
   */
  uint8_t nat_pmp_enabled;
  /**
   * Pointer to an array of null-terminated UTF-8 CIDRs to bypass
   * (see `--bypass-cidr`). Length given by `bypass_cidrs_count`.
   */
  const char *const *bypass_cidrs;
  /**
   * Number of entries in `bypass_cidrs`.
   */
  uint32_t bypass_cidrs_count;
  /**
   * Signed multi-hop directory JSON, fetched by Swift over URLSession
   * from `GET {api}/v1/multihop/directory`. Required: the production fleet
   * is multi-hop only, so the tunnel always rides the multi-hop wire
   * protocol, a 2-hop circuit when `multihop_two_hop` is 1, otherwise a
   * 1-hop circuit collapsed onto one node. The JSON is verified Rust-side
   * against the baked root pin before use. A null directory is a malformed
   * config and the tunnel fails to start.
   */
  const char *multihop_directory_json;
  /**
   * 1 selects a 2-hop circuit (entry != exit, country diverse); 0 a
   * 1-hop circuit. Ignored when `multihop_directory_json` is null.
   */
  uint8_t multihop_two_hop;
  /**
   * Optional ISO 3166-1 alpha-2 entry-country hint (null / empty = any).
   */
  const char *multihop_entry_country;
  /**
   * Optional ISO 3166-1 alpha-2 exit-country hint (null / empty = any).
   */
  const char *multihop_exit_country;
  /**
   * Optional path to the App Group file that persists the multi-hop
   * directory anti-rollback high-water mark (highest trusted
   * `generation`). Read before verification to reject a stale directory,
   * raised after a successful selection. Null disables persistence (the
   * gate then only protects within a single connect). Ignored when
   * `multihop_directory_json` is null.
   */
  const char *multihop_generation_state_path;
  /**
   * Optional path to the App Group file that persists the exit-pubkey
   * trust-on-first-use (TOFU) pin table. When non-null, the selected
   * exit's Ed25519 pubkey is checked against the pin for its `exit_id`
   * before connecting: a mismatch fails the connection closed and is
   * surfaced via `warren_tunnel_take_pin_mismatch` for the user to trust
   * or reject. Null disables pinning.
   */
  const char *pin_store_path;
} WarrenTunnelParametersC;

/**
 * Tunnel status snapshot.
 */
typedef struct WarrenTunnelStatusC {
  enum WarrenTunnelStateC state;
  uint64_t bytes_in;
  uint64_t bytes_out;
  /**
   * Seconds since the current connection was established. 0 when
   * `state != Connected`.
   */
  uint64_t connected_duration_seconds;
  /**
   * Cumulative failover count this session.
   */
  uint32_t failover_count;
} WarrenTunnelStatusC;

/**
 * Tagged-union event payload.
 * The Swift side reads `tag` first then accesses the matching
 * `data_*` field (e.g. `data_failover_country_code` when tag ==
 * `Failover`). cbindgen emits this as a C struct with a discriminator.
 */
typedef struct WarrenTunnelEventC {
  enum WarrenTunnelEventTagC tag;
  /**
   * Failover : null-terminated UTF-8 country code of new exit.
   */
  const char *data_failover_country_code;
  /**
   * NatPmp* : forwarded port (external).
   */
  uint16_t data_nat_pmp_external_port;
  /**
   * NatPmpMapped : internal port + lifetime.
   */
  uint16_t data_nat_pmp_internal_port;
  uint32_t data_nat_pmp_lifetime_seconds;
  /**
   * NatPmpFailed : null-terminated UTF-8 reason.
   */
  const char *data_nat_pmp_failure_reason;
} WarrenTunnelEventC;

/**
 * Event callback signature. Called from a Tokio task on the
 * warren-tunnel runtime ; the Swift side must marshal back to the
 * MainActor if the callback updates UI state.
 *
 * `event` pointer is owned by Rust for the duration of the callback
 * only ; Swift must copy any UTF-8 strings before returning.
 */
typedef void (*WarrenTunnelEventCallback)(const struct WarrenTunnelEventC *event, void *context);

/**
 * Outbound packet callback signature. Called from a Tokio task that
 * drains [`warrenguard_transport::IosTun`] after each `PacketDevice::send` from
 * the downlink pump. The Swift side bridges to
 * `NEPacketTunnelFlow.writePackets(_:withProtocols:)`.
 *
 * `data` + `len` are owned by Rust for the duration of the call ;
 * Swift must copy before returning. `context` is the opaque pointer
 * passed at registration time.
 */
typedef void (*WarrenTunnelOutboundCallback)(const uint8_t *data, uintptr_t len, void *context);

/**
 * Exit-allocated IPv4 surfaced to Swift after the multi-hop circuit's
 * setup-stream returns an `IpAssign` control message. Swift re-applies
 * `NEPacketTunnelNetworkSettings` with this address so the TUN's source
 * IP matches what the exit expects, otherwise return traffic is dropped
 * (the iOS analog of the daemon's `RealTun::reassign_ipv4`).
 *
 * IPv4-only: the iOS multi-hop path keeps native IPv6 blackholed
 * (`wants_ipv6 = false`), so no v6 assignment is requested or surfaced.
 */
typedef struct WarrenTunnelIpAssignC {
  /**
   * Exit-allocated IPv4 address (network byte order, i.e. octets a.b.c.d).
   */
  uint8_t ipv4[4];
  /**
   * Subnet prefix length for the allocated address.
   */
  uint8_t prefix_len;
  /**
   * Exit-side gateway IPv4 (the exit TUN host).
   */
  uint8_t gateway_ipv4[4];
} WarrenTunnelIpAssignC;

/**
 * Exit-allocated IP callback signature. Called from a Tokio task on the
 * warren-tunnel runtime when the multi-hop circuit reports a fresh
 * `IpAssign`. The Swift side re-applies the tunnel network settings.
 *
 * `assign` is owned by Rust for the duration of the call; Swift must
 * copy the fields before returning.
 */
typedef void (*WarrenTunnelIpAssignCallback)(const struct WarrenTunnelIpAssignC *assign,
                                             void *context);

typedef struct SwiftApiContext {
  const struct ApiContext *_0;
} SwiftApiContext;

typedef struct SwiftAccessMethodSettingsWrapper {
  struct SwiftAccessMethodSettingsContext *_0;
} SwiftAccessMethodSettingsWrapper;

/**
 * Opaque wrapper around domain fronting configuration, created by
 * [`new_domain_fronting_config`] and consumed by the init functions.
 */
typedef struct SwiftDomainFrontingConfig {
  struct DomainFrontingConfigContext *_0;
} SwiftDomainFrontingConfig;

typedef struct SwiftShadowsocksLoaderWrapperContext {
  const void *shadowsocks_loader;
} SwiftShadowsocksLoaderWrapperContext;

typedef struct SwiftShadowsocksLoaderWrapper {
  struct SwiftShadowsocksLoaderWrapperContext _0;
} SwiftShadowsocksLoaderWrapper;

typedef struct SwiftData {
  void *ptr;
} SwiftData;

typedef struct SwiftCancelHandle {
  struct RequestCancelHandle *ptr;
} SwiftCancelHandle;

typedef struct SwiftRetryStrategy {
  struct RetryStrategy *_0;
} SwiftRetryStrategy;

typedef struct SwiftMullvadApiResponse {
  uint8_t *body;
  uintptr_t body_size;
  char *etag;
  uint16_t status_code;
  char *error_description;
  char *server_response_code;
  bool success;
} SwiftMullvadApiResponse;

typedef struct CompletionCookie {
  void *inner;
} CompletionCookie;

typedef struct SwiftServerMock {
  const void *server_ptr;
  const void *mock_ptr;
  uint16_t port;
} SwiftServerMock;

/**
 * Callback function type for logging.
 * - `level`: The log level (1=Error, 2=Warn, 3=Info, 4=Debug, 5=Trace)
 * - `message`: Null-terminated UTF-8 string containing the log message
 */
typedef void (*LogCallback)(uint8_t level, const char *message);

/**
 * Starts a Warren tunnel with the given parameters. Returns an opaque
 * handle on success, or null on failure (invalid parameters, tunnel
 * feature disabled at build time, runtime allocation failure).
 *
 * `packet_fd` is the iOS `NEPacketTunnelFlow` file descriptor. iOS
 * does *not* expose the TUN fd directly through `NEPacketTunnelFlow` ;
 * pass `-1` and use [`warren_tunnel_inject_inbound_packet`] +
 * [`warren_tunnel_set_outbound_callback`] for the Swift bridge.
 *
 * # Safety
 * `parameters` must point to a valid `WarrenTunnelParametersC` (all
 * inner pointers valid for the duration of this call).
 */
struct WarrenTunnelHandle *warren_tunnel_start(const struct WarrenTunnelParametersC *parameters,
                                               int32_t _packet_fd);

/**
 * Stops the tunnel and releases all resources. Idempotent : safe to
 * call on a null handle (no-op).
 *
 * # Safety
 * `handle` must have been returned by [`warren_tunnel_start`] and must
 * not have been stopped already.
 */
void warren_tunnel_stop(struct WarrenTunnelHandle *handle);

/**
 * Called on iOS `sleep()`. Deliberately does not stop the pump: iOS suspends
 * the whole extension process shortly after `sleep()`, which halts the pump on
 * its own, and while the extension is briefly backgrounded but not yet
 * suspended the pump MUST keep running to hold the connection. So this leaves
 * the live `Connected` state untouched; [`warren_tunnel_resume`] health-checks
 * the session on wake. A userspace pump-pause would only risk stalling a
 * still-running backgrounded tunnel for no benefit.
 *
 * Returns `0` on success, `-1` on null handle.
 *
 * # Safety
 * `handle` must be a valid pointer from [`warren_tunnel_start`].
 */
int warren_tunnel_pause(struct WarrenTunnelHandle *handle);

/**
 * Resumes after a [`warren_tunnel_pause`]. Health-checked: it never reports
 * Connected without a live session. A short suspension usually leaves the
 * Quinn session intact; a long one (peer idle-timeout) drops it, so resume
 * forces a redial and stays Reconnecting until the supervisor republishes a
 * session. Idempotent.
 *
 * Returns `0` on success, `-1` on null handle.
 *
 * # Safety
 * `handle` must be a valid pointer from [`warren_tunnel_start`].
 */
int warren_tunnel_resume(struct WarrenTunnelHandle *handle);

/**
 * Triggers a tunnel reconnect (e.g. on Wi-Fi <-> cellular handover): forces
 * the supervisor to redial its current circuit on a fresh socket, under the
 * supervisor's own configured backoff. TUN, routes and killswitch stay up.
 *
 * Returns `0` on success, `-3` if the tunnel is not connected.
 *
 * # Safety
 * `handle` must be a valid pointer from [`warren_tunnel_start`].
 */
int warren_tunnel_reconnect(struct WarrenTunnelHandle *handle);

/**
 * Reports a network path change (Wi-Fi to cellular and back) from the Swift
 * `NWPathMonitor` observer to the migration watchdog, which rebinds the live
 * QUIC endpoint and revalidates the path instead of re-handshaking. Escalates
 * to the existing reconnect path when the path cannot be revalidated, so this
 * never replaces the Swift state machine, it front-runs it.
 *
 * Handle-free on purpose: the observer watches the whole extension and has no
 * tunnel handle. A no-op when no tunnel session is running.
 */
void warren_tunnel_notify_path_change(void);

/**
 * Reads the current tunnel status into `out_status`.
 *
 * Returns `0` on success, `-1` on invalid input.
 *
 * # Safety
 * `handle` may be null (status reports `Disconnected`). When non-null
 * it must be a valid pointer from [`warren_tunnel_start`].
 * `out_status` must point to a writable `WarrenTunnelStatusC`.
 */
int warren_tunnel_status(struct WarrenTunnelHandle *handle, struct WarrenTunnelStatusC *out_status);

/**
 * Registers a callback invoked on tunnel events (connected,
 * disconnected, reconnecting, failover, NAT-PMP events).
 *
 * Replaces any previously registered callback. Passing a null
 * callback clears the registration.
 *
 * Returns `0` on success.
 *
 * # Safety
 * `handle` must be a valid pointer from [`warren_tunnel_start`].
 * `callback` (if non-null) must outlive the call to
 * [`warren_tunnel_stop`]. `context` is passed back unchanged ; lifetime
 * is the caller's responsibility.
 */
int warren_tunnel_set_event_callback(struct WarrenTunnelHandle *handle,
                                     WarrenTunnelEventCallback callback,
                                     void *context);

/**
 * Registers a callback that ships outbound IP packets to Swift for
 * `NEPacketTunnelFlow.writePackets`. Replaces any previously
 * registered callback.
 *
 * Returns `0` on success, `-1` on null handle.
 *
 * # Safety
 * Same invariants as [`warren_tunnel_set_event_callback`].
 */
int warren_tunnel_set_outbound_callback(struct WarrenTunnelHandle *handle,
                                        WarrenTunnelOutboundCallback callback,
                                        void *context);

/**
 * Registers a callback invoked when the multi-hop circuit reports a
 * fresh exit-allocated IPv4 (`IpAssign`). The Swift side re-applies the
 * `NEPacketTunnelNetworkSettings` with the new address. Replaces any
 * previously registered callback. Passing a null callback is rejected
 * (use a no-op from Swift to clear).
 *
 * Returns `0` on success, `-1` on null handle.
 *
 * # Safety
 * Same invariants as [`warren_tunnel_set_event_callback`].
 */
int warren_tunnel_set_ip_assign_callback(struct WarrenTunnelHandle *handle,
                                         WarrenTunnelIpAssignCallback callback,
                                         void *context);

/**
 * Pushes an inbound IP packet onto the tunnel uplink queue. Called
 * by Swift after each `NEPacketTunnelFlow.readPackets` completion.
 *
 * `data` is borrowed for the duration of this call ; Rust copies the
 * bytes before returning.
 *
 * Returns `0` on success, `-1` on null handle / null data / zero
 * length.
 *
 * # Safety
 * `handle` must be a valid pointer from [`warren_tunnel_start`].
 * `data` must point to at least `len` bytes of readable memory.
 */
int warren_tunnel_inject_inbound_packet(struct WarrenTunnelHandle *handle,
                                        const uint8_t *data,
                                        uintptr_t len);

/**
 * Verifies a freshly fetched multi-hop directory and returns its trusted
 * `generation`, or `-1` on any verification / expiry / rollback failure.
 * Handle-free: used by the Swift periodic-refresh loop to decide whether
 * the fleet changed (a higher generation than the running session's) and a
 * re-selection is warranted, without disturbing the live tunnel.
 *
 * Does NOT raise the persisted anti-rollback high-water mark (it only reads
 * it for the rollback gate); the mark is raised only on a successful
 * connect, so a periodic check of an inflated-generation forgery cannot
 * poison it. `generation_state_path` may be null (gate then reads as 0).
 *
 * # Safety
 * `directory_json` must be a valid null-terminated UTF-8 C string.
 * `generation_state_path`, when non-null, must be a valid null-terminated
 * UTF-8 C string.
 */
int64_t warren_multihop_check_generation(const char *directory_json,
                                         const char *generation_state_path);

/**
 * Take (and clear) the JSON details of the last exit-pubkey TOFU mismatch
 * recorded on `handle`, if any. Returns a heap C string
 * `{"exit_id","observed","pinned","country"}` the caller MUST free via
 * `warren_wallet_free_mnemonic`, or null when there is no pending mismatch.
 * Swift calls this after a connection failure to decide whether to present
 * the Trust / Report / Reject alert.
 *
 * # Safety
 * `handle` must be null or a live pointer returned by
 * [`warren_tunnel_start`] and not yet stopped.
 */
char *warren_tunnel_take_pin_mismatch(struct WarrenTunnelHandle *handle);

/**
 * Trust a (possibly new) exit pubkey for `exit_id`, overwriting any
 * existing pin in the App Group store at `pin_store_path`. Called when the
 * user accepts a mismatch ("Trust new key") or to pre-seed a pin. All
 * string args are null-terminated hex / UTF-8. Returns 0 on success,
 * -1 on invalid input.
 *
 * # Safety
 * Each non-null pointer must be a valid null-terminated C string.
 */
int warren_pin_trust(const char *pin_store_path,
                     const char *exit_id_hex,
                     const char *pubkey_hex,
                     const char *country_code);

/**
 * Clear all exit-pubkey pins in the App Group store at `pin_store_path`.
 * Backs the Settings "Reset pinned exit keys" action. Returns the number
 * of pins dropped (>= 0), or -1 on invalid input.
 *
 * # Safety
 * `pin_store_path` must be a valid null-terminated C string.
 */
int64_t warren_pin_reset(const char *pin_store_path);

/**
 * The compiled product environment's anchor table as a heap-allocated JSON
 * C string: one object whose keys are the columns of
 * `fixtures/client-rules/product_env.json` (`deep_link_scheme`,
 * `connect_host`, `forum_public_url`, `api_url`, `application_id`, ...), so
 * Swift reads the scheme and the hosts from the Rust reference instead of
 * spelling them again. Null only if the table could not be rendered, which
 * the crate's own tests rule out.
 *
 * The returned pointer must be passed to `warren_product_anchors_free`
 * exactly once.
 */
char *warren_product_anchors(void);

/**
 * Frees a string previously returned by `warren_product_anchors`. No-op on
 * null.
 *
 * # Safety
 * `ptr` must have been returned by `warren_product_anchors` and must not
 * have been freed already.
 */
void warren_product_anchors_free(char *ptr);

/**
 * Generates a new BIP39 mnemonic with `word_count` words (12 or 24).
 *
 * Returns a heap-allocated C string. Caller MUST free via
 * `warren_wallet_free_mnemonic`. Returns null on error (invalid
 * word_count or RNG failure).
 *
 * # Safety
 * The returned pointer must be passed back to
 * `warren_wallet_free_mnemonic` exactly once. Reading the string after
 * freeing is undefined behaviour.
 */
char *warren_wallet_generate_mnemonic(uint32_t word_count);

/**
 * Frees a mnemonic string previously returned by
 * `warren_wallet_generate_mnemonic`. No-op on null.
 *
 * # Safety
 * `ptr` must have been returned by `warren_wallet_generate_mnemonic`
 * and must not have been freed already.
 */
void warren_wallet_free_mnemonic(char *ptr);

/**
 * Derives the Warren identity 32-byte seed from a BIP39 mnemonic.
 *
 * `mnemonic` : null-terminated UTF-8 BIP39 phrase (12 or 24 words).
 * `out_seed` : caller-provided buffer of at least 32 bytes ; written
 * on success.
 *
 * Returns `0` on success, `-1` on invalid input.
 *
 * # Safety
 * `mnemonic` must point to a valid null-terminated C string.
 * `out_seed` must point to a writable buffer of at least 32 bytes.
 */
int warren_wallet_seed_from_mnemonic(const char *mnemonic, uint8_t *out_seed);

/**
 * Derives the Ed25519 public key from a 32-byte seed.
 *
 * `seed` : caller-provided buffer of 32 bytes.
 * `out_pubkey` : caller-provided buffer of at least 32 bytes ; written
 * on success.
 *
 * Returns `0` on success, `-1` on invalid input.
 *
 * # Safety
 * Both `seed` and `out_pubkey` must point to writable buffers of at
 * least 32 bytes each.
 */
int warren_wallet_derive_pubkey(const uint8_t *seed, uint8_t *out_pubkey);

/**
 * Derives the **Warren SS58 address** (`wb…`, network prefix 13295)
 * from a 32-byte seed.
 *
 * This is the canonical string form of the Warren wallet identity - the
 * value shown in the UI, copied to the clipboard, and carried in the
 * `X-Warren-PubKey` request header. The same algorithm
 * (`warren_identity::ss58`) is used by the daemon and the backend
 * verifier, so the address round-trips byte-for-byte. It is the iOS
 * analog of Android's `pubkey_ss58_from_mnemonic`
 * (`warren-jni/src/wallet.rs`).
 *
 * `seed` : caller-provided buffer of 32 bytes.
 *
 * Returns a heap-allocated C string holding the SS58 address. Caller
 * MUST free it via `warren_wallet_free_mnemonic` (the free routine is
 * type-agnostic: it reclaims any `CString` produced by this crate).
 * Returns null on invalid input (null seed) or internal error.
 *
 * # Safety
 * `seed` must point to a readable buffer of at least 32 bytes. The
 * returned pointer must be passed back to `warren_wallet_free_mnemonic`
 * exactly once.
 */
char *warren_wallet_pubkey_ss58(const uint8_t *seed);

/**
 * Signs an arbitrary payload with the Ed25519 signing key derived
 * from `seed`.
 *
 * `seed` : 32-byte seed buffer.
 * `payload` : pointer to `payload_len` bytes.
 * `out_signature` : caller-provided buffer of at least 64 bytes ;
 * written on success.
 *
 * Returns `0` on success, `-1` on invalid input, `-2` on internal
 * signing error.
 *
 * # Safety
 * All pointers must be non-null and point to buffers of the documented
 * sizes.
 */
int warren_wallet_sign(const uint8_t *seed,
                       const uint8_t *payload,
                       uintptr_t payload_len,
                       uint8_t *out_signature);

/**
 * Verify a signed update manifest and evaluate it against the running app
 * version.
 *
 * `manifest` / `manifest_len` : raw bytes of the fetched `ios.json`.
 * `current_version` : null-terminated running app version string
 * (e.g. `2026.3` or `2026.3-dev1`).
 *
 * Returns a heap-allocated JSON C string
 * `{"supported":bool,"latest_version":"X.Y.Z"}` when the manifest signature
 * and expiry verify (`latest_version` is omitted when the manifest lists no
 * releases). Returns null when verification fails (bad signature, expired
 * metadata, unparseable manifest, invalid input): the caller must then treat
 * the manifest as absent, never trust its content.
 *
 * An unparseable `current_version` yields `supported: true` (fail-open: a
 * version-string surprise must not lock users out), while dev builds are
 * always supported per the shared rule.
 *
 * # Safety
 * `manifest` must point to `manifest_len` readable bytes. `current_version`
 * must be a valid null-terminated C string. The returned pointer must be
 * passed to `warren_version_check_free` exactly once.
 */
char *warren_version_check_verify(const uint8_t *manifest,
                                  uintptr_t manifest_len,
                                  const char *current_version);

/**
 * Frees a string previously returned by `warren_version_check_verify`.
 * No-op on null.
 *
 * # Safety
 * `ptr` must have been returned by `warren_version_check_verify` and must
 * not have been freed already.
 */
void warren_version_check_free(char *ptr);

/**
 * Signed `GET /v1/subscription`. Returns the wallet's subscription
 * expiry as `{"ok":true,"expires_at":<unix secs>}` or an error envelope.
 *
 * # Safety
 * `seed`, when non-null, must point to at least 32 readable bytes. The
 * returned pointer must be freed once via `warren_wallet_free_mnemonic`.
 */
char *warren_account_get_subscription(const uint8_t *seed);

/**
 * Signed `POST /v1/payments/apple/init`. Mints an ephemeral payment
 * session bound to the wallet pubkey and returns the session UUID the
 * app must pass to StoreKit as the `appAccountToken`. Returns
 * `{"ok":true,"app_account_token":"<uuid>"}` or an error envelope.
 *
 * # Safety
 * `seed`, when non-null, must point to at least 32 readable bytes. The
 * returned pointer must be freed once via `warren_wallet_free_mnemonic`.
 */
char *warren_account_storekit_init(const uint8_t *seed);

/**
 * Signed `POST /v1/payments/apple/check`. Uploads the StoreKit 2
 * signed transaction JWS so the backend can verify it against Apple's
 * root CA and credit the wallet's subscription. Returns
 * `{"ok":true,"expires_at":<unix secs>}` or an error envelope. The JWS
 * is never logged.
 *
 * # Safety
 * `seed`, when non-null, must point to at least 32 readable bytes;
 * `jws`, when non-null, must be a valid null-terminated C string. The
 * returned pointer must be freed once via `warren_wallet_free_mnemonic`.
 */
char *warren_account_storekit_check(const uint8_t *seed, const char *jws);

/**
 * Unsigned `POST /v1/register`. Binds the wallet pubkey to a new
 * subscription via a voucher secret. Returns
 * `{"ok":true,"expires_at":<unix secs>}` or an error envelope. The
 * voucher secret is never logged.
 *
 * # Safety
 * `seed`, when non-null, must point to at least 32 readable bytes;
 * `voucher`, when non-null, must be a valid null-terminated C string.
 * The returned pointer must be freed once via
 * `warren_wallet_free_mnemonic`.
 */
char *warren_account_redeem_voucher(const uint8_t *seed, const char *voucher);

/**
 * Signed `DELETE /v1/account`. Permanently deletes the wallet's
 * subscription server-side. Returns `{"ok":true}` or an error envelope.
 *
 * # Safety
 * `seed`, when non-null, must point to at least 32 readable bytes. The
 * returned pointer must be freed once via `warren_wallet_free_mnemonic`.
 */
char *warren_account_delete(const uint8_t *seed);

/**
 * Sign and submit a forum-login challenge for `sid` to the connect `host`.
 *
 * Derives the `WarrenIdentity` from the 32-byte wallet `seed`, reads the
 * session's status once (a dead session is `expired` without a signature
 * spent; the answer's `Date` corrects the device clock), builds the signed
 * `POST /v1/forum/login` request at the corrected time (host allowlist + sid
 * shape checked in `crate::forum`), sends it, and returns the outcome
 * envelope: `{"ok":true,...}` with the forum identity the broker handed back,
 * `subscription-required` on 403, `clock-skew` on connect's 401 token,
 * `expired` on 404, `error` with a `reason` class for anything else (input,
 * build, runtime, transport, an unnamed status). Nothing about the request
 * (seed, sid, signature, nonce) is ever logged.
 *
 * # Safety
 * `seed`, when non-null, must point to at least 32 readable bytes; `sid` and
 * `host` must be valid NUL-terminated C strings. The returned pointer must be
 * freed exactly once via `warren_wallet_free_mnemonic`.
 */
char *warren_forum_login(const uint8_t *seed, const char *sid, const char *host);

/**
 * Best-effort: notify the connect `host` that the user declined the forum login
 * for `sid` (`POST /v1/session/<sid>/cancel`), so the waiting browser page
 * unblocks instead of polling to timeout. Unsigned (no seed / wallet material);
 * mirrors the desktop `cancelForumLogin`. Failures are ignored (connect drops
 * a login session on its own after 5 minutes, the `pending_ttl_secs.login` of
 * `fixtures/client-rules/forum_link.json`). Blocking; call off the main thread.
 *
 * # Safety
 * `sid` and `host` must be valid NUL-terminated C strings.
 */
void warren_forum_cancel(const char *sid, const char *host);

/**
 * Called by Swift to set the available access methods
 */
void mullvad_api_update_access_methods(struct SwiftApiContext api_context,
                                       struct SwiftAccessMethodSettingsWrapper settings_wrapper);

/**
 * Called by Swift to update the currently used access methods
 *
 * # SAFETY
 * `access_method_id` must point to a null terminated string in a UUID format
 *
 */
void mullvad_api_use_access_method(struct SwiftApiContext api_context,
                                   const char *access_method_id);

/**
 * Called by Swift to trigger a fetching and caching of addresses
 *
 * # SAFETY
 *
 * this takes no arguments other than the API context. The API context
 * needs to be valid, and the function should not be called concurrently.
 */
void mullvad_api_update_address_cache(struct SwiftApiContext swift_api_context);

/**
 * Creates a [`SwiftDomainFrontingConfig`] that owns copies of the provided strings.
 *
 * # Safety
 *
 * Both `front` and `proxy_host` must be pointers to null-terminated strings.
 * The pointers only need to be valid for the duration of this call.
 */
struct SwiftDomainFrontingConfig new_domain_fronting_config(const char *front,
                                                            const char *proxy_host);

/**
 * # Safety
 *
 * `host` must be a pointer to a null terminated string representing a hostname for Mullvad API host.
 * This hostname will be used for TLS validation but not used for domain name resolution.
 *
 * `address` must be a pointer to a null terminated string representing a socket address through which
 * the Mullvad API can be reached directly.
 *
 * If a context cannot be constructed this function will panic since the call site would not be able
 * to proceed in a meaningful way anyway.
 *
 * This function is safe.
 */
struct SwiftApiContext mullvad_api_init_new_tls_disabled(const char *host,
                                                         const char *address,
                                                         const char *encrypted_dns_domain,
                                                         struct SwiftDomainFrontingConfig domain_fronting,
                                                         struct SwiftShadowsocksLoaderWrapper bridge_provider,
                                                         struct SwiftAccessMethodSettingsWrapper settings_provider,
                                                         void (*access_method_change_callback)(const void*,
                                                                                               const uint8_t*),
                                                         const void *access_method_change_context);

/**
 * # Safety
 *
 * `host` must be a pointer to a null terminated string representing a hostname for Mullvad API host.
 * This hostname will be used for TLS validation but not used for domain name resolution.
 *
 * `address` must be a pointer to a null terminated string representing a socket address through which
 * the Mullvad API can be reached directly.
 *
 * access_method_change_callback is a function with the C calling convention which will be called
 * whenever the access method changes with a user-specified opaque pointer and a pointer to the bytes
 * of the access method's UUID. Note that this callback must remain valid for the lifetime of the
 * program.
 *
 * access_method_change_context is the pointer passed verbatim to the callback. It is not dereferenced
 * by the Rust code, but remains opaque.
 *
 * If a context cannot be constructed this function will panic since the call site would not be able
 * to proceed in a meaningful way anyway.
 *
 * This function is safe.
 */
struct SwiftApiContext mullvad_api_init_new(const char *host,
                                            const char *address,
                                            const char *encrypted_dns_domain,
                                            struct SwiftDomainFrontingConfig domain_fronting,
                                            struct SwiftShadowsocksLoaderWrapper bridge_provider,
                                            struct SwiftAccessMethodSettingsWrapper settings_provider,
                                            void (*access_method_change_callback)(const void*,
                                                                                  const uint8_t*),
                                            const void *access_method_change_context);

/**
 * # Safety
 *
 * `host` must be a pointer to a null terminated string representing a hostname for Mullvad API host.
 * This hostname will be used for TLS validation but not used for domain name resolution.
 *
 * `address` must be a pointer to a null terminated string representing a socket address through which
 * the Mullvad API can be reached directly.
 *
 * If a context cannot be constructed this function will panic since the call site would not be able
 * to proceed in a meaningful way anyway.
 *
 * This function is safe.
 */
struct SwiftApiContext mullvad_api_init_inner(const char *host,
                                              const char *address,
                                              const char *encrypted_dns_domain,
                                              struct SwiftDomainFrontingConfig domain_fronting,
                                              bool disable_tls,
                                              struct SwiftShadowsocksLoaderWrapper bridge_provider,
                                              struct SwiftAccessMethodSettingsWrapper settings_provider,
                                              void (*access_method_change_callback)(const void*,
                                                                                    const uint8_t*),
                                              const void *access_method_change_context);

extern void swift_store_address_cache(const uint8_t *data, uint64_t data_size);

extern struct SwiftData swift_read_address_cache(void);

/**
 * Converts parameters into a `Box<AccessMethodSetting>` raw representation that
 * can be passed across the FFI boundary
 *
 * # SAFETY:
 * `unique_identifier` and `name` must point to valid memory regions and contain NULL terminators.
 * They are only valid for the duration of this call.
 *
 * `proxy_configuration` can be NULL, or must be a pointer gotten through
 * either the `convert_shadowsocks` or `convert_socks5` methods.
 */
void *convert_builtin_access_method_setting(const char *unique_identifier,
                                            const char *name,
                                            bool is_enabled,
                                            SwiftAccessMethodKind method_kind,
                                            const void *proxy_configuration);

/**
 * Creates a wrapper around a `Settings` object that can be safely sent across the FFI boundary.
 *
 * # SAFETY
 * `direct_method_raw`, `bridges_method_raw`, `encrypted_dns_method_raw` and
 * `domain_fronting_method_raw` must be raw pointers resulting from a call to
 * `convert_builtin_access_method_setting`.
 * `custom_methods_raw` is an array of pointers to instances of `AccessMethodSetting`
 */
struct SwiftAccessMethodSettingsWrapper init_access_method_settings_wrapper(const void *direct_method_raw,
                                                                            const void *bridges_method_raw,
                                                                            const void *encrypted_dns_method_raw,
                                                                            const void *domain_fronting_method_raw,
                                                                            const void *custom_methods_raw,
                                                                            uintptr_t custom_method_count);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `account_number` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_account(struct SwiftApiContext api_context,
                                                 void *completion_cookie,
                                                 struct SwiftRetryStrategy retry_strategy,
                                                 const char *account_number);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_create_account(struct SwiftApiContext api_context,
                                                    void *completion_cookie,
                                                    struct SwiftRetryStrategy retry_strategy);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `account_number` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_delete_account(struct SwiftApiContext api_context,
                                                    void *completion_cookie,
                                                    struct SwiftRetryStrategy retry_strategy,
                                                    const char *account_number);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_addresses(struct SwiftApiContext api_context,
                                                   void *completion_cookie,
                                                   struct SwiftRetryStrategy retry_strategy);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_api_addrs_available(struct SwiftApiContext api_context,
                                                         void *completion_cookie,
                                                         struct SwiftRetryStrategy retry_strategy,
                                                         const void *access_method_setting);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `etag` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_relays(struct SwiftApiContext api_context,
                                                void *completion_cookie,
                                                struct SwiftRetryStrategy retry_strategy,
                                                const char *etag);

/**
 * Called by the Swift side to signal that a Mullvad API call should be cancelled.
 * After this call, the cancel token is no longer valid.
 *
 * # Safety
 *
 * `handle_ptr` must be pointing to a valid instance of `SwiftCancelHandle`.
 */
void mullvad_api_cancel_task(struct SwiftCancelHandle *handle_ptr);

/**
 * Called by the Swift side to signal that the Rust `SwiftCancelHandle` can be safely
 * dropped from memory.
 *
 * # Safety
 *
 * `handle_ptr` must be pointing to a valid instance of `SwiftCancelHandle`.
 */
void mullvad_api_cancel_task_drop(struct SwiftCancelHandle *handle_ptr);

/**
 * Maps to `mullvadApiCompletionFinish` on Swift side to facilitate callback based completion flow when doing
 * network calls through Mullvad API on Rust side.
 *
 * # Safety
 *
 * `response` must be pointing to a valid instance of `SwiftMullvadApiResponse`.
 *
 * `completion_cookie` must be pointing to a valid instance of `CompletionCookie`. `CompletionCookie` is safe
 * because the pointer in `MullvadApiCompletion` is valid for the lifetime of the process where this type is
 * intended to be used.
 */
extern void mullvad_api_completion_finish(struct SwiftMullvadApiResponse response,
                                          struct CompletionCookie completion_cookie);

/**
 * Get device info via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_ios_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_ios_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * the `account_number` must be a pointer to a null terminated string.
 * the `identifier` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_device(struct SwiftApiContext api_context,
                                                void *completion_cookie,
                                                struct SwiftRetryStrategy retry_strategy,
                                                const char *account_number,
                                                const char *identifier);

/**
 * Get devices info via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * the `account_number` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_devices(struct SwiftApiContext api_context,
                                                 void *completion_cookie,
                                                 struct SwiftRetryStrategy retry_strategy,
                                                 const char *account_number);

/**
 * create device via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * the `account_number` must be a pointer to a null terminated string.
 * the `identifier` must be a pointer to a null terminated string.
 * the `public_key` pointer must be a valid pointer to 32 unsigned bytes.
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_create_device(struct SwiftApiContext api_context,
                                                   void *completion_cookie,
                                                   struct SwiftRetryStrategy retry_strategy,
                                                   const char *account_number,
                                                   const uint8_t *public_key);

/**
 * delete device via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * the `account_number` must be a pointer to a null terminated string.
 * the `identifier` must be a pointer to a null terminated string.
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_delete_device(struct SwiftApiContext api_context,
                                                   void *completion_cookie,
                                                   struct SwiftRetryStrategy retry_strategy,
                                                   const char *account_number,
                                                   const char *identifier);

/**
 * rotate device key via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * the `account_number` must be a pointer to a null terminated string.
 * the `identifier` must be a pointer to a null terminated string.
 * the `public_key` pointer must be a valid pointer to 32 unsigned bytes.
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_rotate_device_key(struct SwiftApiContext api_context,
                                                       void *completion_cookie,
                                                       struct SwiftRetryStrategy retry_strategy,
                                                       const char *account_number,
                                                       const char *identifier,
                                                       const uint8_t *public_key);

/**
 * Converts parameters into a boxed `Shadowsocks` configuration that is safe
 * to send across the FFI boundary
 *
 * # SAFETY
 * `address` must be a pointer to at least `address_len` bytes.
 * `c_password` and `c_cipher` must be pointers to null terminated strings
 */
const void *new_shadowsocks_access_method_setting(const uint8_t *address,
                                                  uintptr_t address_len,
                                                  uint16_t port,
                                                  const char *c_password,
                                                  const char *c_cipher);

/**
 * Converts parameters into a boxed `Socks5Remote` configuration that is safe
 *
 * to send across the FFI boundary
 *
 * # SAFETY
 * `address` must be a pointer to at least `address_len` bytes.
 * `c_username` and `c_password` must be pointers to null terminated strings, or null
 */
const void *new_socks5_access_method_setting(const uint8_t *address,
                                             uintptr_t address_len,
                                             uint16_t port,
                                             const char *c_username,
                                             const char *c_password);

char *get_shadowsocks_chipers(void);

/**
 * Deallocates a CString returned by the Mullvad API client.
 *
 * # Safety
 *
 * `cstr_ptr` must be a pointer to a string allocated by another `mullvad_api` function.
 */
void mullvad_api_cstring_drop(char *cstr_ptr);

/**
 * # Safety
 *
 * `method` must be a pointer to a null terminated string representing the http method.
 *
 * `path` must be a pointer to a null terminated string representing the url path.
 *
 * `response_code` must be a usize representing the http response code.
 *
 * `response_body` must be a pointer to a null terminated string representing the body.
 *
 * This function is safe.
 */
struct SwiftServerMock mullvad_api_mock_get(const char *path,
                                            uintptr_t response_code,
                                            const uint8_t *response_body);

/**
 * # Safety
 *
 * `path` must be a pointer to a null terminated string representing the url path.
 *
 * `response_code` must be a usize representing the http response code.
 *
 * `match_body` must be a pointer to a null terminated json string representing the body the server expects.
 *
 * This function is safe.
 */
struct SwiftServerMock mullvad_api_mock_post(const char *path,
                                             uintptr_t response_code,
                                             const char *match_body);

/**
 * Called by the Swift side to signal that the Rust `SwiftServerMock` can be safely
 * dropped from memory.
 *
 * # Safety
 *
 * `mock_ptr` must be pointing to a valid instance of `SwiftServerMock`. This function
 * is not safe to call multiple times with the same `SwiftServerMock`.
 */
void mullvad_api_mock_drop(struct SwiftServerMock mock_ptr);

/**
 * Called by the Swift side to signal that the Rust `SwiftMullvadApiResponse` can be safely
 * dropped from memory.
 *
 * # Safety
 *
 * `response` must be pointing to a valid instance of `SwiftMullvadApiResponse`. This function
 * is not safe to call multiple times with the same `SwiftMullvadApiResponse`.
 */
void mullvad_response_drop(struct SwiftMullvadApiResponse response);

/**
 * Creates a retry strategy that never retries after failure.
 * The result needs to be consumed.
 */
struct SwiftRetryStrategy mullvad_api_retry_strategy_never(void);

/**
 * Creates a retry strategy that retries `max_retries` times with a constant delay of `delay_sec`.
 * The result needs to be consumed.
 */
struct SwiftRetryStrategy mullvad_api_retry_strategy_constant(uintptr_t max_retries,
                                                              uint64_t delay_sec);

/**
 * Creates a retry strategy that retries `max_retries` times with a exponantially increating delay.
 * The delay will never exceed `max_delay_sec`
 * The result needs to be consumed.
 */
struct SwiftRetryStrategy mullvad_api_retry_strategy_exponential(uintptr_t max_retries,
                                                                 uint64_t initial_sec,
                                                                 uint32_t factor,
                                                                 uint64_t max_delay_sec);

/**
 * Creates a `Shadowsocks` configuration.
 *
 * # SAFETY
 * `rawBridgeProvider` **must** be provided by a call to `init_swift_shadowsocks_loader_wrapper`
 * It is okay to persist it, and use it across multiple threads.
 */
extern const void *swift_get_shadowsocks_bridges(const void *rawBridgeProvider);

/**
 * Called by the Swift side in order to provide an object to rust that can create
 * Shadowsocks configurations
 *
 * # SAFETY
 * `shadowsocks_loader` **must be** pointing to a valid instance of a `SwiftShadowsocksBridgeProvider`
 * That instance's lifetime has to be equivalent to a `'static` lifetime in Rust
 * This function does not take ownership of `shadowsocks_loader`
 */
struct SwiftShadowsocksLoaderWrapper init_swift_shadowsocks_loader_wrapper(const void *shadowsocks_loader);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `account_number` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_init_storekit_payment(struct SwiftApiContext api_context,
                                                           void *completion_cookie,
                                                           struct SwiftRetryStrategy retry_strategy,
                                                           const char *account_number);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * `body` must be a pointer to a contiguous memory segment
 *
 * `body_size` must be the size of the body
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_check_storekit_payment(struct SwiftApiContext api_context,
                                                            void *completion_cookie,
                                                            struct SwiftRetryStrategy retry_strategy,
                                                            const uint8_t *body,
                                                            uintptr_t body_size);

extern uint8_t *swift_data_get_ptr(const struct SwiftData *data);

extern uintptr_t swift_data_get_len(const struct SwiftData *data);

extern void swift_data_drop(struct SwiftData *data);

/**
 * Initialize the Rust logger with a Swift callback.
 *
 * This function should be called once early in the application lifecycle,
 * before any Rust code that uses logging is invoked.
 *
 * # Safety
 * - `callback` must be a valid function pointer that remains valid for the lifetime of the program.
 * - This function is safe to call multiple times, but only the first call will have an effect.
 */
void init_rust_logging(LogCallback callback);
