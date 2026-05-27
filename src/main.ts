import { createApp } from "vue";
import { createPinia } from "pinia";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import zhCn from "element-plus/dist/locale/zh-cn.mjs";
import VxeUITable from "vxe-table";
import "vxe-table/lib/style.css";
import VxeUIPcUI from "vxe-pc-ui";
import "vxe-pc-ui/lib/style.css";

import App from "./App.vue";
import router from "./router";
import "./styles/global.scss";

const app = createApp(App);

app.use(createPinia());
app.use(router);
app.use(ElementPlus, { locale: zhCn });
app.use(VxeUITable);
app.use(VxeUIPcUI);

app.mount("#app");
