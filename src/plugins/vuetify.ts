// src/plugins/vuetify.ts
import "vuetify/styles";
import { createVuetify } from "vuetify";
import * as components from "vuetify/components";
import * as directives from "vuetify/directives";
import { mdi } from "vuetify/iconsets/mdi";
import "@mdi/font/css/materialdesignicons.css"; // 确保图标样式被加载

// Studio Bold 风格色板（md/页面风格.md）
// 深色区 #1A1A1A + #0D0D0D，浅色区 #F5F5F0，珊瑚色 #FF6B6B 只出现在视线着陆点
const vuetify = createVuetify({
  components,
  directives,
  icons: {
    defaultSet: "mdi",
    sets: { mdi },
  },
  theme: {
    defaultTheme: "dark",
    themes: {
      dark: {
        dark: true,
        colors: {
          primary: "#FF6B6B",
          secondary: "#333333",
          background: "#1A1A1A",
          surface: "#1A1A1A",
          "surface-bright": "#333333",
          "surface-dark": "#0D0D0D",
          "surface-light": "#F5F5F0",
          error: "#E55A5A",
          success: "#4CAF7D",
          info: "#5B8DEF",
          warning: "#F5A623",
        },
      },
      light: {
        dark: false,
        colors: {
          primary: "#E55A5A",
          secondary: "#E5E5DC",
          background: "#F5F5F0",
          surface: "#F5F5F0",
          "surface-bright": "#EDEDE6",
          "surface-dark": "#1A1A1A",
          "surface-light": "#1A1A1A",
          error: "#D64545",
          success: "#3E8E63",
          info: "#3B6FD4",
          warning: "#C77F1B",
        },
      },
    },
  },
  defaults: {
    VBtn: { rounded: 0, density: "comfortable" },
    VCard: { rounded: 0 },
    VDialog: { scrim: "rgba(13,13,13,0.72)" },
  },
});

export default vuetify;
