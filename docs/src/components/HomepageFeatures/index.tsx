import React from 'react';
import clsx from 'clsx';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  description: JSX.Element;
};

const FeatureList: FeatureItem[] = [
  {
    title: '热重加载',
    description: (
      <>无需重启程序即可应用更改</>
    ),
  },
  {
    title: '灵活配置',
    description: (
      <>代理可以随意嵌套, 支持TCP和UDP</>
    ),
  },
  {
    title: 'JSON Schema 生成',
    description: (
      <>无需查文档, 通过代码补全直接编写配置文件</>
    ),
  },
];

function Feature({ title, description }: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center padding-horiz--md">
        <h3>{title}</h3>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): JSX.Element {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
