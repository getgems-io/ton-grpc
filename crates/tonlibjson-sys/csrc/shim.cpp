namespace td {
void clear_thread_locals();
}

extern "C" void td_clear_thread_locals() {
  td::clear_thread_locals();
}
