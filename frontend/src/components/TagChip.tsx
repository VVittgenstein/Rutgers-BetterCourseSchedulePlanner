import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { classNames } from '../utils/classNames';
import './TagChip.css';

export type TagChipTone = 'default' | 'info' | 'success' | 'warning' | 'danger';

export interface TagChipProps {
  label: string;
  value?: string;
  tone?: TagChipTone;
  active?: boolean;
  compact?: boolean;
  icon?: ReactNode;
  onClick?: () => void;
  onRemove?: () => void;
}

export function TagChip({
  label,
  value,
  tone = 'default',
  active = true,
  compact = false,
  icon,
  onClick,
  onRemove,
}: TagChipProps) {
  const { t } = useTranslation();
  const Element = onClick ? 'button' : 'span';

  return (
    <Element
      type={onClick ? 'button' : undefined}
      className={classNames(
        'tag-chip',
        `tag-chip--${tone}`,
        active ? 'tag-chip--active' : 'tag-chip--inactive',
        compact && 'tag-chip--compact',
        onClick && 'tag-chip--interactive',
      )}
      onClick={onClick}
    >
      {icon && <span className="tag-chip__icon">{icon}</span>}
      <span className="tag-chip__text">
        <span className="tag-chip__label">{label}</span>
        {value && <span className="tag-chip__value">{value}</span>}
      </span>
      {onRemove && (
        <button
          type="button"
          aria-label={t('tagChip.remove', { label })}
          className="tag-chip__remove"
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            onRemove();
          }}
        >
          ×
        </button>
      )}
    </Element>
  );
}
