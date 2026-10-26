// The page's one accent, keyed to the user's signature stat (the stat the ability is built on).
// The six hues are Araki's signature colours (hot pink, yellow, light blue, green, purple, orange) and sit on the chart so that the
// opposite axes are complementary pairs: Power/Stamina pink and green, Speed/Precision yellow and purple, Range/Potential blue and orange.
// Each has a light-ground and a dark-ground value. The label on the filled button is the ground colour, so a value that passes
// WCAG AA (4.5:1) against the ground passes for the button label too. tools/site_test/selftest.js and design_check.py verify it.
export const ACCENTS = {
  PWR: { light: '#b0104c', dark: '#ff6da3' },
  SPD: { light: '#7a5f00', dark: '#f0c02c' },
  RNG: { light: '#17659a', dark: '#6cbcf0' },
  STA: { light: '#1d7438', dark: '#5ccd80' },
  PRC: { light: '#6a3cb4', dark: '#b69aff' },
  DEV: { light: '#a94700', dark: '#ff964a' },
};
export const GROUND = { light: '#ffffff', dark: '#0e0e0e' };
// the ground of the drop zone, the inputs and the table header
export const GROUND2 = { light: '#f4f4f4', dark: '#1a1a1a' };

const lin = (c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const luminance = (hex) => {
  const [r, g, b] = [1, 3, 5].map((i) => lin(parseInt(hex.slice(i, i + 2), 16) / 255));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
export const contrast = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

// "PWR".."DEV" from the analysis: the signature stat, or for a balanced chart the stat that stood out most; none when nothing was graded
export function accentKey(analysis) {
  const s = analysis && analysis.ability && analysis.ability.signature;
  const key = s && (s.stat === 'BAL' ? s.top : s.stat);
  return key in ACCENTS ? key : null;
}
