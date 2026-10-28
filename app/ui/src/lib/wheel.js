// A wheel drawn for a table by the screens (the make-media window's Wheel).

/** A wheel drawn for a table (a front end's round logo), from its backglass picture, else its
 *  playfield's: the picture in a circle, darkened toward its foot, ringed in the app's colours,
 *  the table's name across it — a PNG's bytes. */
export async function drawWheel(t) {
  const src = t.picture.backglass || t.picture.playfield;
  const size = 512;
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const g = canvas.getContext('2d');
  const c = size / 2;
  const r = 236;
  await document.fonts.load('700 80px "Barlow Condensed"');
  g.save();
  g.beginPath();
  g.arc(c, c, r, 0, Math.PI * 2);
  g.clip();
  g.fillStyle = '#0d1016';
  g.fillRect(0, 0, size, size);
  if (src) {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.src = src;
    await img.decode().catch(() => {});
    if (img.naturalWidth) {
      // the picture filling the circle, its top third the likeliest to hold the title
      const scale = Math.max(size / img.naturalWidth, size / img.naturalHeight);
      const w = img.naturalWidth * scale;
      const h = img.naturalHeight * scale;
      g.drawImage(img, (size - w) / 2, Math.min(0, (size - h) / 3), w, h);
    }
  }
  const shade = g.createLinearGradient(0, 0, 0, size);
  shade.addColorStop(0, 'rgba(8,10,15,0.05)');
  shade.addColorStop(1, 'rgba(8,10,15,0.6)');
  g.fillStyle = shade;
  g.fillRect(0, 0, size, size);
  // the name on a dark band across the middle, a line or two, as large as fits
  const words = t.short.toUpperCase().split(/\s+/);
  let lines = [words.join(' ')];
  let px = 92;
  const fits = (ls, p) => {
    g.font = `700 ${p}px "Barlow Condensed", "Arial Narrow", sans-serif`;
    return ls.every((l) => g.measureText(l).width <= 400);
  };
  while (!fits(lines, px) && px > 40) px -= 4;
  if (!fits(lines, px) && words.length > 1) {
    const half = Math.ceil(words.length / 2);
    lines = [words.slice(0, half).join(' '), words.slice(half).join(' ')];
    px = 84;
    while (!fits(lines, px) && px > 30) px -= 4;
  }
  const lineH = px * 0.95;
  const bandH = lineH * lines.length + 34;
  g.fillStyle = 'rgba(8,10,15,0.72)';
  g.fillRect(0, c - bandH / 2, size, bandH);
  g.textAlign = 'center';
  g.textBaseline = 'middle';
  g.shadowColor = 'rgba(76,194,255,0.55)';
  g.shadowBlur = 14;
  g.fillStyle = '#ffffff';
  lines.forEach((l, i) => g.fillText(l, c, c + (i - (lines.length - 1) / 2) * lineH + px * 0.04));
  g.restore();
  // the ring
  const ring = g.createLinearGradient(c - r, 0, c + r, 0);
  ring.addColorStop(0, '#b48cff');
  ring.addColorStop(0.5, '#7fa8ff');
  ring.addColorStop(1, '#4cc2ff');
  g.lineWidth = 14;
  g.strokeStyle = ring;
  g.beginPath();
  g.arc(c, c, r - 4, 0, Math.PI * 2);
  g.stroke();
  g.lineWidth = 2;
  g.strokeStyle = 'rgba(255,255,255,0.4)';
  g.beginPath();
  g.arc(c, c, r - 4, 0, Math.PI * 2);
  g.stroke();
  const blob = await new Promise((ok) => canvas.toBlob(ok, 'image/png'));
  return new Uint8Array(await blob.arrayBuffer());
}
