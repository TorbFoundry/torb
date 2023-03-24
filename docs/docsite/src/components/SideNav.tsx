import React from 'react';
import { useRouter } from 'next/router';
import Link from 'next/link';

const items = [
  {
    title: 'Torb Foundry',
    links: [
      { href: '/docs', children: 'Overview' },
      { href: '/docs/getting-started-cli', children: 'CLI' },
    ],
  },
  {
    title: 'V1 Specs',
    links: [
      { href: "/docs/specs", children: "Specs" },
      { href: '/docs/specs/stacks-v1', children: 'Stacks' },
      { href: '/docs/specs/services-v1', children: 'Services' },
      { href: '/docs/specs/projects-v1', children: 'Projects' },
    ],
  },
];

export function SideNav() {
  const router = useRouter();

  return (
    <nav className="sidenav">
      {items.map((item) => (
        <details key={item.title}>
          <summary>{item.title}</summary>
          <ul className="flex column">
            {item.links.map((link) => {
              const active = router.pathname === link.href;
              return (
                <li key={link.href} className={active ? 'active' : ''}>
                  <Link {...link} />
                </li>
              );
            })}
          </ul>
        </details>
      ))}
      <style jsx>
        {`
          details > summary {
            list-style-type: '▶️';
          }
          details[open] > summary {
            list-style-type: '🔽';
          }
          nav {
            position: sticky;
            top: var(--top-nav-height);
            height: calc(100vh - var(--top-nav-height));
            flex: 0 0 auto;
            overflow-y: auto;
            padding: 2.5rem 2rem 2rem;
            border-right: 1px solid var(--border-color);
          }
          span {
            font-size: larger;
            font-weight: 500;
            padding: 0.5rem 0 0.5rem;
          }
          ul {
            padding: 0;
          }
          li {
            list-style: none;
            margin: 0;
          }
          li :global(a) {
            text-decoration: none;
          }
          li :global(a:hover),
          li.active :global(a) {
            text-decoration: underline;
          }
        `}
      </style>
    </nav>
  );
}
