import {
  useId,
  type ButtonHTMLAttributes,
  type ReactNode,
} from 'react';

export interface ActionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly busy?: boolean;
  readonly busyLabel?: string;
  readonly tone?: 'default' | 'accent' | 'quiet' | 'danger' | 'danger-outline';
}

export function ActionButton({
  busy = false,
  busyLabel,
  children,
  className,
  disabled,
  tone = 'default',
  type = 'button',
  ...props
}: ActionButtonProps) {
  const classes = ['bcsp-action', `bcsp-action--${tone}`, className]
    .filter(Boolean)
    .join(' ');
  return (
    <button
      {...props}
      aria-busy={busy || undefined}
      className={classes}
      disabled={disabled || busy}
      type={type}
    >
      {busy ? (busyLabel ?? children) : children}
    </button>
  );
}

export interface StatePanelProps {
  readonly action?: ReactNode;
  readonly detail: ReactNode;
  readonly heading: ReactNode;
  readonly kind: 'loading' | 'empty' | 'error';
  readonly marker?: ReactNode;
}

/* Text glyphs inside the 40px marker circle (spec 4.13): loading ⟳, empty –, error ! */
const defaultMarkers: Readonly<Record<StatePanelProps['kind'], string>> = {
  loading: '⟳',
  empty: '–',
  error: '!',
};

export function StatePanel({ action, detail, heading, kind, marker }: StatePanelProps) {
  return (
    <section
      aria-busy={kind === 'loading' || undefined}
      aria-live={kind === 'error' ? 'assertive' : 'polite'}
      className={`bcsp-state-panel bcsp-state-panel--${kind}`}
      data-state={kind}
      role={kind === 'error' ? 'alert' : 'status'}
    >
      <div aria-hidden="true" className="bcsp-state-panel__marker">
        {marker ?? defaultMarkers[kind]}
      </div>
      <div className="bcsp-state-panel__body">
        <h2 className="bcsp-state-panel__title">{heading}</h2>
        <p className="bcsp-state-panel__detail">{detail}</p>
        {action === undefined ? null : <div className="bcsp-state-panel__action">{action}</div>}
      </div>
    </section>
  );
}

export interface StatusSignalProps {
  readonly detail?: ReactNode;
  readonly label: ReactNode;
  readonly state: 'ready' | 'refreshing' | 'stale' | 'offline' | 'unknown';
}

export function StatusSignal({ detail, label, state }: StatusSignalProps) {
  return (
    <div className="bcsp-status-signal" data-state={state}>
      <span aria-hidden="true" className="bcsp-status-signal__mark" />
      <span>
        <span className="bcsp-status-signal__label">{label}</span>
        {detail === undefined ? null : (
          <span className="bcsp-status-signal__detail">{detail}</span>
        )}
      </span>
    </div>
  );
}

export interface MetricProps {
  readonly detail?: ReactNode;
  readonly label: ReactNode;
  readonly unit?: ReactNode;
  readonly value: ReactNode;
}

export function Metric({ detail, label, unit, value }: MetricProps) {
  return (
    <dl className="bcsp-metric">
      <dt className="bcsp-metric__label">{label}</dt>
      <dd className="bcsp-metric__value">
        {value}
        {unit === undefined ? null : <span className="bcsp-metric__unit">{unit}</span>}
      </dd>
      {detail === undefined ? null : <dd className="bcsp-metric__detail">{detail}</dd>}
    </dl>
  );
}

export interface FieldRenderProps {
  readonly controlProps: {
    readonly 'aria-describedby'?: string;
    readonly 'aria-errormessage'?: string;
    readonly 'aria-invalid'?: true;
    readonly id: string;
    readonly required?: true;
  };
}

export interface FieldFrameProps {
  readonly children: (props: FieldRenderProps) => ReactNode;
  readonly error?: ReactNode;
  readonly helper?: ReactNode;
  readonly label: ReactNode;
  readonly required?: boolean;
}

export function FieldFrame({ children, error, helper, label, required = false }: FieldFrameProps) {
  const id = useId();
  const helperId = helper === undefined ? undefined : `${id}-helper`;
  const errorId = error === undefined ? undefined : `${id}-error`;
  const describedBy = [helperId, errorId].filter(Boolean).join(' ') || undefined;
  return (
    <div className="bcsp-field">
      <label className="bcsp-field__label" htmlFor={id}>{label}</label>
      {children({
        controlProps: {
          ...(describedBy === undefined ? {} : { 'aria-describedby': describedBy }),
          ...(errorId === undefined ? {} : {
            'aria-errormessage': errorId,
            'aria-invalid': true,
          }),
          id,
          ...(required ? { required: true } : {}),
        },
      })}
      {helper === undefined ? null : <p className="bcsp-field__helper" id={helperId}>{helper}</p>}
      {error === undefined ? null : <p className="bcsp-field__error" id={errorId}>{error}</p>}
    </div>
  );
}

export function VisuallyHidden({ children }: { readonly children: ReactNode }) {
  return <span className="bcsp-visually-hidden">{children}</span>;
}
