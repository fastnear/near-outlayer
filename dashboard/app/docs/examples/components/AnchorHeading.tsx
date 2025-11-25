// Anchor heading component with clickable link
export function AnchorHeading({ id, children, badges }: { id: string; children: React.ReactNode; badges?: React.ReactNode }) {
  const handleClick = (e: React.MouseEvent) => {
    e.preventDefault();
    const element = document.getElementById(id);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'start' });
      window.history.pushState(null, '', `#${id}`);
    }
  };

  return (
    <h3 className="text-2xl font-semibold group relative">
      <a href={`#${id}`} onClick={handleClick} className="hover:text-[var(--primary-orange)] transition-colors">
        {children}
        {badges}
        <span className="absolute -left-6 opacity-0 group-hover:opacity-100 transition-opacity text-gray-400">#</span>
      </a>
    </h3>
  );
}
