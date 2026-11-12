// Appearance settings that every part of the UI needs (theme, animations, blur).
// Stored in Orbit's settings.json; applied here to <html> as data-* attributes,
// which app.css uses to switch colors and turn effects off.

export const ui = $state({
  theme: "black",
  animations: true,
  blur: true,
});

export function applyUi(s: { theme: string; animations: boolean; blur: boolean }) {
  ui.theme = s.theme;
  ui.animations = s.animations;
  ui.blur = s.blur;
  const root = document.documentElement;
  root.dataset.theme = s.theme;
  root.dataset.animations = s.animations ? "on" : "off";
  root.dataset.blur = s.blur ? "on" : "off";
}

/** Animation length for Svelte transitions: 0 when animations are off. */
export function ms(duration: number) {
  return ui.animations ? duration : 0;
}
