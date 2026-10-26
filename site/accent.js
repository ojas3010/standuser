// The page's one accent is the exact ground colour of the user's card (the colour the card renderer fills its ground with, read back
// from the card's own SVG so the two can never differ), and the text on it is the card's ink, which the card palette keeps at 4.5:1
// or better on that ground. Before a card exists the accent is the page's ink colour (style.css). tools/site_test/selftest.js and
// design_check.py verify it.
const hexOf = (svg, cls) => {
  const m = svg.match(new RegExp(`\\.f-${cls}\\{fill:(#[0-9a-fA-F]{6})\\}`));
  return m ? m[1].toLowerCase() : null;
};
// { ground, ink } of a card SVG (the light or the dark one, not the auto one, which carries both)
export const cardColours = (svg) => ({ ground: hexOf(svg, 'bg'), ink: hexOf(svg, 'ink') });

const lin = (c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const luminance = (hex) => {
  const [r, g, b] = [1, 3, 5].map((i) => lin(parseInt(hex.slice(i, i + 2), 16) / 255));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
export const contrast = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};
