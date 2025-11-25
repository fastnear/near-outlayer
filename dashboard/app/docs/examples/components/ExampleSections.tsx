import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

export function UseCasesSection({ items }: { items: (string | React.ReactNode)[] }) {
  return (
    <>
      <h4 className="font-semibold mt-4 mb-2">Use Cases:</h4>
      <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
        {items.map((item, idx) => (
          <li key={idx}>{item}</li>
        ))}
      </ul>
    </>
  );
}

export function KeyFeaturesSection({ items }: { items: (string | React.ReactNode)[] }) {
  return (
    <>
      <h4 className="font-semibold mt-4 mb-2">Key Features:</h4>
      <ul className="list-disc list-inside text-gray-700 space-y-1 mb-4">
        {items.map((item, idx) => (
          <li key={idx}>{item}</li>
        ))}
      </ul>
    </>
  );
}

export function TechnicalDetailsSection({ items }: { items: (string | React.ReactNode)[] }) {
  return (
    <>
      <h4 className="font-semibold mt-4 mb-2">Technical Details:</h4>
      <ul className="list-disc list-inside text-gray-700 mb-4 space-y-1 text-sm">
        {items.map((item, idx) => (
          <li key={idx}>{item}</li>
        ))}
      </ul>
    </>
  );
}

export function CodeExampleSection({ title, code, language = 'json', description }: {
  title: string;
  code: string;
  language?: string;
  description?: React.ReactNode;
}) {
  return (
    <div className="mb-4">
      {title && <p className="text-sm font-medium text-gray-800 mb-1">{title}</p>}
      <SyntaxHighlighter language={language} style={vscDarkPlus} customStyle={{ borderRadius: '0.5rem', fontSize: '0.875rem' }}>
        {code}
      </SyntaxHighlighter>
      {description && <p className="text-sm text-gray-600 mt-1">{description}</p>}
    </div>
  );
}

export function HowItWorksSection({ items }: { items: (string | React.ReactNode)[] }) {
  return (
    <div className="mt-4 p-4 bg-blue-50 border-l-4 border-blue-400">
      <h4 className="font-semibold text-gray-900 mb-2">How It Works</h4>
      <ol className="list-decimal list-inside text-sm text-gray-700 space-y-1">
        {items.map((item, idx) => (
          <li key={idx}>{item}</li>
        ))}
      </ol>
    </div>
  );
}

export function LearnMoreSection({ children }: { children: React.ReactNode }) {
  return (
    <div className="mt-4 p-4 bg-gray-50 rounded-lg">
      <h4 className="font-semibold text-gray-900 mb-2">Learn More</h4>
      {children}
    </div>
  );
}

export function SecurityNotesSection({ items }: { items: (string | React.ReactNode)[] }) {
  return (
    <div className="mt-4 p-4 bg-yellow-50 border-l-4 border-yellow-400">
      <h4 className="font-semibold text-gray-900 mb-2">Important Security Notes</h4>
      <ul className="text-sm text-gray-700 space-y-1">
        {items.map((item, idx) => (
          <li key={idx}>{item}</li>
        ))}
      </ul>
    </div>
  );
}
