import { createPinia } from "pinia";
import { createPersistedState } from "pinia-plugin-persistedstate";

const pinia = createPinia();

// Namespace every persisted store inside the webview's localStorage so it
// cannot collide with unrelated data stored for the same origin.
pinia.use(
  createPersistedState({
    key: (id) => `comics-reader:${id}`,
  }),
);

export default pinia;
