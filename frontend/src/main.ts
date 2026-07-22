/**
 * Application Entry Point
 *
 * Vue ， Pinia 、Vue Router ， DOM。
 * Initializes the Vue application, mounts Pinia state management and Vue Router,
 * then mounts the app to the DOM.
 */

import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import i18n from "./i18n";
import "./style.css";
import "vue-sonner/style.css";

// （reka-ui） wheel/touchstart  passive
// Fix third-party libraries (reka-ui) not marking wheel/touchstart listeners as passive
const _addEventListener = EventTarget.prototype.addEventListener;
EventTarget.prototype.addEventListener = function (type, listener, options) {
	if (type === "wheel" || type === "touchstart") {
		const opts = options === undefined || options === null
			? { passive: true }
			: typeof options === "object"
				? { passive: true, ...options }
				: options;
		return _addEventListener.call(this, type, listener, opts);
	}
	return _addEventListener.call(this, type, listener, options);
};

// Vue ， Pinia ， #app
// Create Vue app instance, register Pinia and router, mount to #app element
createApp(App).use(createPinia()).use(router).use(i18n).mount("#app");
