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
// The page's field (the band behind the header and the stage) is the card's ground, with the card's own ink and muted ink for the text on
// it, so the text keeps the 4.5:1 the card palette guarantees. Its dots, its big faint ゴ and the flash of the reveal are the ground moved
// toward the tone that is away from the ink (white for black ink, black for white ink), so none of them can lower the text's contrast.
const mix = (a, b, t) => '#' + [1, 3, 5].map((i) => Math.round(parseInt(a.slice(i, i + 2), 16) * (1 - t) + parseInt(b.slice(i, i + 2), 16) * t).toString(16).padStart(2, '0')).join('');

const lin = (c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const luminance = (hex) => {
  const [r, g, b] = [1, 3, 5].map((i) => lin(parseInt(hex.slice(i, i + 2), 16) / 255));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
export const contrast = (a, b) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

export const fieldColours = (svg) => {
  const { ground, ink } = cardColours(svg);
  const away = luminance(ink) < 0.5 ? '#ffffff' : '#000000';
  return { ground, ink, mute: hexOf(svg, 'mute'), dot: mix(ground, away, 0.1), ghost: mix(ground, away, 0.14), flash: mix(ground, away, 0.55) };
};
