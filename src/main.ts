import { createApp } from "vue";
import App from "./App.vue";
import { getAppRelease } from "./services/appRelease";

void getAppRelease().catch((error: unknown) =>
  console.error("Could not load the app release on launch:", error),
);
createApp(App).mount("#app");
