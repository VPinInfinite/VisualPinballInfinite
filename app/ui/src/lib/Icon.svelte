<script>
  // One of the screens' icons, stroked in the text's colour (or filled, for PLAY's triangle).
  import { ic } from './icons.js';

  let { name, size = 18, fill = false, width = 2 } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={fill ? 'currentColor' : 'none'}
  stroke={fill ? 'none' : 'currentColor'}
  stroke-width={width}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={ic[name]} />
</svg>
