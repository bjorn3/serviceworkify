// jshint esversion: 6
// jshint worker: true

var CACHE = "{site}";

self.addEventListener('install', function (evt) {
    console.log('The service worker is being installed.');
    caches.keys().then(function (keys) {
        for (let cache of keys) {
            caches.delete(cache);
        }
    });
});

self.addEventListener('fetch', function (evt) {
    console.log('The service worker is serving the asset.');

    let res = update(evt.request)
        .catch(function () {
            return fromCache(evt.request);
        })
        .catch(function (err) {
            console.error(err);
            return new Response("<h1>Offline: " + err + "</h1>", {
                status: 503, statusText: "Offline",
                headers: { "Content-Type": "text/html" }
            });
        });

    evt.respondWith(res);
});

function fromCache(request) {
    return caches.open(CACHE).then(function (cache) {
        return cache.match(request);
    }).then(function (matching) {
        return matching || Promise.reject('no-match ' + request.url);
    });
}

function update(request) {
    return fetch(request).then(function (response) {
        let r = response.clone();
        return caches.open(CACHE).then(function (cache) {
            cache.put(request, response);
            return r;
        });
    });
}