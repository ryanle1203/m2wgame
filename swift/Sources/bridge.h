// C interface of the Rust core (src/ffi.rs).
// One call: JSON command in, JSON answer out. Free the answer with wb_free.
char *wb_request(const char *request);
void wb_free(char *ptr);
