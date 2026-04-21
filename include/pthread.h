#ifndef _PTHREAD_H
#define _PTHREAD_H

typedef unsigned long long pthread_t;
typedef void *pthread_attr_t;
typedef void *pthread_mutex_t;
typedef void *pthread_mutexattr_t;

int pthread_create(pthread_t *thread, pthread_attr_t *attr, void *(*start)(void *), void *arg);
int pthread_join(pthread_t thread, void **retval);
int pthread_mutex_init(pthread_mutex_t *mutex, pthread_mutexattr_t *attr);
int pthread_mutex_lock(pthread_mutex_t *mutex);
int pthread_mutex_unlock(pthread_mutex_t *mutex);

#endif
