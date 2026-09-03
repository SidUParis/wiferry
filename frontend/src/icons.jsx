export function Icon({ name, size = 20 }) {
  const paths = {
    add: <path d="M12 5v14M5 12h14" />,
    arrowDown: <><path d="M12 3v12" /><path d="m7 11 5 5 5-5" /><path d="M5 21h14" /></>,
    arrowUp: <><path d="M12 21V9" /><path d="m7 13 5-5 5 5" /><path d="M5 3h14" /></>,
    check: <path d="m5 12 4 4L19 6" />,
    close: <><path d="m6 6 12 12" /><path d="m18 6-12 12" /></>,
    copy: <><rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></>,
    devices: <><rect x="3" y="5" width="14" height="10" rx="2" /><path d="M8 19h4M10 15v4" /><rect x="17" y="9" width="4" height="8" rx="1" /></>,
    file: <><path d="M6 2h8l4 4v16H6z" /><path d="M14 2v5h5" /></>,
    folder: <path d="M3 6h7l2 2h9v11H3z" />,
    link: <><path d="M10 13a5 5 0 0 0 7.1.1l2-2a5 5 0 0 0-7.1-7.1l-1.1 1.1" /><path d="M14 11a5 5 0 0 0-7.1-.1l-2 2A5 5 0 0 0 12 20l1.1-1.1" /></>,
    refresh: <><path d="M20 6v6h-6" /><path d="M19 12a7 7 0 1 1-2-5" /></>,
    shield: <><path d="M12 2 20 6v6c0 5-3.5 8-8 10-4.5-2-8-5-8-10V6z" /><path d="m8.5 12 2.2 2.2 4.8-5" /></>,
    stop: <rect x="6" y="6" width="12" height="12" rx="1" />,
    wifi: <><path d="M3 9a14 14 0 0 1 18 0" /><path d="M6 13a9 9 0 0 1 12 0" /><path d="M9.5 17a4 4 0 0 1 5 0" /><circle cx="12" cy="20" r="1" fill="currentColor" stroke="none" /></>,
  };
  return (
    <svg
      aria-hidden="true"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {paths[name] || paths.file}
    </svg>
  );
}
